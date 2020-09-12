#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/*

indent(+ind, -ind, -tok)
indent(i0::ind, i0::ind, [ind, c]) -> i0 indent(ind, ind, [ind, c])
indent([], [i*], [+ind, c]) -> hsp+(i*) term(c)
indent([], (' '+i*)::-ind, +inda::+inda::-tok) -> '-' hsp+(i*) indent([], -ind, +inda::-tok)
indent([], (' '+i*)::-ind, [+inda, c]) -> '-' hsp+(i*) indent([], -ind, [ind, c])
indent([], [], [ind, c]) -> term(c)
indent(i0::i1::ind, [], -ind::-tok) -> indent(i1::ind, [], -tok)
indent([i0], [], [-ind, c]) -> term(c)
indent(+ind, -ind, -tok) -> hsp(i*) '\n' indent(+ind, -ind, -tok)

hsp+(out) :- (' ' or '\t') (' ' or '\t')*
term(c) :- nonws(c), c != '-'
*/

typedef struct parser {
	char c;
	int  row, col, n;
	char* line;
	int   linez;
	char (*getc)(struct parser*);
	int  (*next)(struct parser*);
	int  (*err)(struct parser*, char*, ...);
	void* errdata;
} parser;

typedef struct str_list {
	char* s;
	struct str_list* next;
} str_list;

enum tok_V { TOK_IND = 0x81, TOK_PIND = 0x82, TOK_PINDA = 0x83, TOK_MIND = 0x84 };
typedef struct tok_list {
	unsigned char tok;
	struct tok_list* next;
} tok_list;

int
ishsp(char c) { return c == ' ' || c == '\t'; }

int
par_hspp(parser* P, char** s, int* sn) {
	int err;
	int sz;

	if (!*s) {
		sz = 4; *sn = 0;
		if (!(*s = malloc(sz))) { P->err(P, "malloc failed"); return 1; }
	}
	while (ishsp(P->c)) {
		while (*sn >= sz && sz*2 > sz) sz *= 2;
		if (sz*2 <= sz) { P->err(P, "reached max str size"); return 1; }
		if (!(*s = realloc(*s, sz))) { P->err(P, "malloc failed"); return 1; }
		*s[*sn] = P->c; (*sn)++;
		if (err = P->next(P)) return err;
	}
	return 0;
}

int
par_expect(parser* P, char c) {
	static char cmsg[] = "unexpected char '_', expected '_'";
	if (P->c != c) {
		cmsg[17] = P->c;
		cmsg[31] = c;
		P->err(P, cmsg);
		return 1;
	}
	return P->next(P);
}

int
par_expects(parser* P, char* s) {
	int err;
	if (!s) return 0;
	while (*s) {
		if (err = par_expect(P, *s)) return err;
		s++;
	}
	return 0;
}


int
par_indent(parser* P, str_list* indin, str_list** indout, tok_list** tokout) {
	int err;
	char* s;
	int sz = 4, sn = 0;
	str_list* last, *ind = indin;
	tok_list* tok;

	while (ind && ishsp(P->c)) {
		if (err = par_expects(P, ind->s)) return err;
		last = ind;
		ind = ind->next;
	}

	// -ind
	if (!ind) {
		last->next = 0;
		while (ind) {
			last = ind;
			ind = ind->next;
			free(last->s); free(last);

			if (!(tok = malloc(sizeof(tok_list)))) { P->err(P, "malloc failed"); return 1; }
			*tok = (tok_list){ TOK_MIND, *tokout };
			*tokout = tok;
		}
		*indout = indin;
		return 0;
	}

	// +ind
	s = 0;
	if (ishsp(P->c)) {
		if (err = par_hspp(P, &s, &sn)) return err;
		if (!(last->next = malloc(sizeof(str_list)))) { P->err(P, "malloc failed"); return 1; }
		*(last->next) = (str_list){ s, 0 };

		if (!(*tokout = malloc(sizeof(tok_list)))) { P->err(P, "malloc failed"); return 1; }
		**tokout = (tok_list){ TOK_PIND, 0 };
		tok = *tokout;
	}

	// +inda
	while (P->c == '-') {
		if (err = P->next(P)) return err;
		if (!ishsp(P->c)) { P->err(P, "expected hsp after '-'"); return 1; }
		if (err = par_hspp(P, &s, &sn)) return err;
		if (!(s = realloc(s, sn+1))) { P->err(P, "malloc failed"); return 1; }
		memmove(s+1, s, sn);
		s[0] = ' ';

		if (!(tok->next = malloc(sizeof(tok_list)))) { P->err(P, "malloc failed"); return 1; }
		tok = tok->next; *tok = (tok_list){ TOK_PINDA, 0 };
	}
	return 0;
}


char
par_getc_stdin(parser* P) {
	return getchar();
}

int
par_next(parser* P) {
	char ind = P->c == '\n';
	if ((P->c = P->getc(P)) < 0) {
		P->err(P, "getc returned error");
		return 1;
	}

	P->n++;
	switch (P->c) {
	case '\n':
		P->row++;
	case '\r':
		P->col = 0;
		break;
	default:
		P->col++;
	}

	if (!P->line) {
		P->linez = 4;
		P->line = malloc(P->linez);
	}
	if (P->linez <= P->col) {
		while (P->linez <= P->col) P->linez *= 2;
		P->line = realloc(P->line, P->linez);
	}
	P->line[P->col-1] = P->c;
	P->line[P->col] = 0;

	return 0;
}

int
par_set_err(parser* P, char* fmt, ...) {
	P->errdata = fmt;
	return 0;
}

int
main(int argc, char* argv[]) {
	parser P = {
		.getc = par_getc_stdin,
		.next = par_next,
		.err = par_set_err,
		.errdata = "unknown error",
	};
	tok_list* toks, *last;
	str_list* inds;
	if (argc > 1 && argv[1][0] == '-' && argv[1][1] == 'g') printf("DEBUG\n");
	while (P.next(&P)) {
		if (par_indent(&P, 0, &inds, &toks)) {
			fprintf(stderr, "%s:%d:%d\n%s\n", P.errdata, P.row, P.col, P.line);
			for (int i=0; i<P.col-1; i++) fprintf(stderr, " "); fprintf(stderr, "^\n");
			return 1;
		}

		while (toks) {
			if (argc > 1 && argv[1][0] == '-' && argv[1][1] == 'g') switch (toks->tok) {
			case TOK_IND: printf("ind "); break;
			case TOK_PIND: printf("+ind "); break;
			case TOK_PINDA: printf("+inda "); break;
			case TOK_MIND: printf("-ind "); break;
			} else
				printf("%c", toks->tok);
			last = toks; toks = toks->next; free(last);
		}
		printf("%c", P.c);
	}
}
