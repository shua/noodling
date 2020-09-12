#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/*
is this lalr?

S -> value
value -> '[' sp elems | '{' sp fields | scalar
scalar -> '"' qstr | freescalar
freescalar -> PRINTABLE_NO_SPECIAL sp | PRINTABLE_NO_SPECIAL freescalar
qstr -> '"' | '\' PRINTABLE qstr | PRINTABLE_NO_DQ qstr
elems -> ']' | value sp ',' sp elems
fields -> '}' | field sp ',' sp fields
field -> key sp ':' sp value
key -> '"' qstr | ident
ident -> ident0 | ident0 ident
ident0 -> ALPHANUM | '_' | '-' | '.'
sp -> eps | sp0 sp
sp0 -> ' ' | '\t' | '\n'

PRINTABLE ::= any printable character
PRINTABLE_NO_DQ ::= any printable character except '"'
PRINTABLE_NO_SPECIAL ::= any printable char except '"', ']', '}', ',', '\n'
ALPHANUM ::= any alpha numeric character
*/

typedef struct parser {
	char c;
	int row, col, n;
	char (*getc)(struct parser*);
	int (*next)(struct parser*);
	int (*err)(struct parser*, char*, ...);
	void *errdata;
	char* line;
	int linez;
} parser;

typedef struct scalar {
	char V;
	union {
		char* s;
		int n;
	};
} scalar;
typedef struct array {
	int n;
	struct value* vs;
} array;
typedef struct object {
	int n;
	char** ks;
	struct value* vs;
} object;
typedef struct value {
	char V;
	union {
		scalar s;
		array a;
		object o;
	};
} value;

int
par_sp(parser* P, int* n) {
	int err;
	while (P->c == ' ' || P->c == '\n' || P->c == '\t') {
		(*n)++;
		if (err = P->next(P)) return err;
	}
	return 0;
}

int
par_qstr(parser* P, char** s) {
	int err, sz = 8, sn = 0, esc = 0;
	*s = malloc(sz);
	while (esc || P->c != '"') {
		if (P->c < 0x20 || P->c > 0x7e) {
			P->err(P, "expected printable character");
			return 1;
		}
		while (sz <= sn) sz *= 2;
		*s = realloc(*s, sz);
		(*s)[sn] = P->c;
		sn++;
		esc = !esc && P->c == '\\';
		if (err = P->next(P)) return err;
	}
	(*s)[sn] = 0;
	return P->next(P);
}

int
par_expect(parser* P, char c) {
	static char msg[] = "unexpected char '_', expected '_'";
	if (P->c != c) {
		msg[17] = P->c;
		msg[31] = c;
		P->err(P, msg);
		return 1;
	}
	return P->next(P);
}

int
par_ident(parser* P, char** s) {
	int err, sz = 8, sn = 0;
	*s = malloc(sz);
	while (
		(P->c >= 'a' && P->c <= 'z')
	||	(P->c >= 'A' && P->c <= 'Z')
	||	(P->c >= '0' && P->c <= '9')
	||	P->c == '_' || P->c == '-' || P->c == '.'
	) {
		while (sz <= sn) sz *= 2;
		*s = realloc(*s, sz);
		(*s)[sn] = P->c;
		sn++;
		if (err = P->next(P)) return err;
	}
	(*s)[sn] = 0;
	return 0;
}

int
par_key(parser* P, char** c) {
	int err;
#define DO(X) if (err = (X)) return err;
	if (P->c == '"') {
		DO(P->next(P));
		return par_qstr(P, c);
	}

	return par_ident(P, c);
#undef DO
}

int par_value(parser* P, value* v);
int
par_scalar(parser* P, scalar* s) {
	int err, sz = 8, sn = 0, trimr = 0, n = 0;
#define DO(X) if (err = (X)) return err;
	if (P->c == '"') {
		s->V = 's';
		DO(P->next(P));
		return par_qstr(P, &(s->s));
	}

	// par_freescalar
	s->V = 'n';
	s->s = malloc(sz);
	while (
		// any printable character
		P->c >= 0x20 && P->c <= 0x7e
		// except value delimeters
	&&	P->c != ']' && P->c != '}' && P->c != ','
		// or dq or nl
	&&	P->c != '"' && P->c != '\n'
	) {
		if (P->c == ' ' || P->c == '\t') trimr++;
		else if (trimr == 0) {
			if  (P->c >= '0' && P->c <= '9') n = n*10 + P->c - '0';
			else if (P->c != '_') s->V = 's';
		} else s->V = 's';
		while (sz <= sn) sz *= 2;
		s->s = realloc(s->s, sz);
		s->s[sn] = P->c;
		sn++;
		DO(P->next(P));
	}

	if (s->V == 'n') {
		free(s->s);
		s->n = n;
	} else {
		s->s[sn-trimr] = 0;
	}
	return 0;
#undef DO
}

int
par_elems(parser* P, array* a) {
	int err, n, first = 1;
#define DO(X) if (err = (X)) return err;
	if (P->c == ']') return P->next(P);
	a->n = 0;
	a->vs = malloc(sizeof(value) * 4);
	for (;;) {
		DO(par_sp(P, &n));
		if (P->c == ']') return P->next(P);
		if (!first) {
			DO(par_expect(P, ','));
			DO(par_sp(P, &n));
		} else first = 0;
		a->n++;
		a->vs = realloc(a->vs, sizeof(value) * a->n);
		DO(par_value(P, &(a->vs[a->n-1])));
	}
#undef DO
}

int
par_fields(parser* P, object* o) {
	int err, n, first = 1;
#define DO(X) if (err = (X)) return err;
	if (P->c == '}') return P->next(P);
	o->n = 0;
	o->ks = malloc(sizeof(char*) * 4);
	o->vs = malloc(sizeof(value) * 4);
	for (;;) {
		DO(par_sp(P, &n));
		if (P->c == '}') return P->next(P);
		if (!first) {
			DO(par_expect(P, ','));
			DO(par_sp(P, &n));
		} else first = 0;
		o->n++;
		o->ks = realloc(o->ks, sizeof(char*) * o->n);
		o->vs = realloc(o->vs, sizeof(char*) * o->n);
		DO(par_key(P, &(o->ks[o->n-1])));
		DO(par_sp(P, &n));
		DO(par_expect(P, ':'));
		DO(par_sp(P, &n));
		DO(par_value(P, &(o->vs[o->n-1])));
	}
#undef DO
}

int
par_value(parser* P, value* v) {
	int err, n;
	switch (P->c) {
	case '[':
		v->V = 'a';
		if (err = P->next(P)) return err;
		if (err = par_sp(P, &n)) return err;
		return par_elems(P, &v->a);
	case '{':
		v->V = 'o';
		if (err = P->next(P)) return err;
		if (err = par_sp(P, &n)) return err;
		return par_fields(P, &v->o);
	default:
		v->V = 's';
		return par_scalar(P, &v->s);
	}
}

int
par_start(parser* P, value* v) {
	int err;
	if (err = P->next(P)) return err;
	return par_value(P, v);
}


char
par_getc_stdin(parser* P) {
	return getchar();
}

int
par_next(parser* P) {
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
}

int
value_print_(value v, int lvl, char compact) {
	int n = 0, n0;
	char* comma = ",\n", * colon = ": ", * beginsp = "\n",
	    * endarray = " ]", * endobject = " }";
	compact = compact > 2 ? 2 : compact < 0 ? 0 : compact;
	switch (compact) { 
	case 1: lvl = -1; comma = ", "; beginsp = " "; break;
	case 2: lvl = -1; comma = ","; colon = ":"; beginsp = ""; endarray = "]"; endobject = "}"; break;
	}
#define DO(X) if (n += (n0 = (X)), n0 < 0) return n0;
	switch (v.V) {

	case 's':
		switch(v.s.V) {
		case 's':
			return printf("\"%s\"", v.s.s);
		case 'n':
			return printf("%d", v.s.n);
		default:
			fprintf(stderr, "unrecognized scalar %c\n", v.s.V);
			return -1;
		}

	case 'a':
		DO(printf("["));
		lvl++;
		for (int i=0; i<v.a.n; i++) {
			DO(printf(i ? comma : beginsp));
			for (int j=0; j<lvl; j++) DO(printf("\t"))
			DO(value_print_(v.a.vs[i], lvl, compact));
		}
		DO(printf(v.a.n ? endarray : "]"));
		return n;

	case 'o':
		DO(printf("{"));
		lvl++;
		for (int i=0; i<v.o.n; i++) {
			DO(printf(i ? comma : beginsp));
			for (int j=0; j<lvl; j++) DO(printf("\t"));
			DO(printf("%s%s", v.o.ks[i], colon));
			DO(value_print_(v.o.vs[i], lvl, compact));
		}
		DO(printf(v.o.n ? endobject : "}"));
		return n;
	default:
		fprintf(stderr, "unrecognized value %c\n", v.V);
		return -1;
	}
#undef DO
}
int value_print(value v) { value_print_(v, 0, 0); }


int
main(int argc, char* argv[]) {
	char compact = 0;
	parser P = {
		.getc = par_getc_stdin,
		.next = par_next,
		.err = par_set_err,
		.errdata = "unknown error",
		0 };
	value V = { 0 };
	if (par_start(&P, &V)) {
		fprintf(stderr, "%s:%d:%d\n%s\n", P.errdata, P.row, P.col, P.line);
		for (int i=0; i<P.col-1; i++) fprintf(stderr, " "); fprintf(stderr, "^\n");
		return 1;
	}

	if (argc > 1 && argv[1][0] == '-' && argv[1][1] == 'c') {
		if (argv[1][2] == 'c') compact = 2;
		else compact = 1;
	}

	value_print_(V, 0, compact); printf("\n");

	return 0;
}
