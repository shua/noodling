#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/*
S -> hsp(n) scalar | hsp(n) fields(n) | hsp(n) elems(n)
fields(ind) -> field(ind) | field(ind) '\n' ind fields(ind)
field(ind)
	-> key hsp ':' hsp scalar
	-> key hsp ':' hsp '\n' ind hsp(1) hsp(n) fields(ind + 1 + n)
	-> key hsp ':' hsp '\n' ind hsp(n) elems(ind + n)
elems(ind) -> elem(ind) | elem(ind) '\n' ind elems(ind)
elem(ind)
	-> '-' hsp(1) hsp scalar
	-> '-' hsp(1) hsp(n) fields(ind + 2 + n)
	-> '-' hsp(1) hsp(n) elems(ind + 2 + n)
scalar -> '"' qstr                             # fill this out
key -> '"' qstr | ident                        # fill this out
qstr -> '"' | '\' PRINTABLE qstr | PRINTABLE_NO_DQ qstr
ident -> ident0 | ident0 ident
ident0 -> ALPHANUM | '_' | '-' | '.'

PRINTABLE ::= any printable character
PRINTABLE_NO_DQ ::= any printable character except '"'
ALPHANUM ::= any alpha numeric character

trying to expand it a little to see far I have to look ahead to disambiguate

S -> hsp(n) ___
	-> scalar
		-> '"' qstr
	-> fields(n)
		-> field(n) -> key hsp ':' hsp ___
			-> scalar
			-> '\n' ind hsp(1) hsp(n1) fields(ind + 1 + n1)
			-> '\n' ind hsp(n1) elems(ind + n1)
		-> field(n) '\n' n fields(n) -> key hsp ':' hsp ___ '\n' n fields(n)
			-> scalar
			-> '\n' ind hsp(1) hsp(n1) fields(ind + 1 + n1)
			-> '\n' ind hsp(n1) elems(ind + n1)
	-> elems(n)
		-> elem(n) -> '-' hsp(1) hsp(n1) ___
			-> scalar
			-> fields(n + 2 + n1)
			-> elems(n + 2 + n1)
		-> elem(n) '\n' n elems(n) -> '-' hsp(1) hsp(n1) ___ '\n' n elems(n)
			-> scalar
			-> fields(n + 2 + n1)
			-> elems(n + 2 + n1)

I think key/scalar will be the difficult thing
also, distinction between a value in the start or element position vs a value in a field

fields(n) -> key hsp ':' hsp fieldval(n) | key hsp ':' hsp fieldval(n) '\n' n fields(n)
fieldval(n)
	-> scalar
	-> '\n' ind hsp(1) hsp(n1) fields(ind + 1 + n1)
	-> '\n' ind hsp(n1) elems(ind + n1)

S -> hsp(n)
	-> scalar
	-> fields(n)
	   field(n) ...
	   key hsp ':' hsp
		-> '\n' n
			-> hsp(n0) hsp(n1)
				-> fields(n + n0 + n1)
				-> elems(n + n0 + n1)
			-> elems(n)
			   elem(n)
			   '-' hsp(n0)
		-> scalar
	-> elems(n)
	   elem(n) ...
	   '-' hsp(n0) hsp(n1)
		-> scalar
		-> fields(n + ' ' + n0 + n1)
		   field(n+2+n1)
		   key hsp ':' hsp ___
		-> elems(n + ' ' + n0 + n1)
		   elem(n+2+n1)
		   '-' hsp(1) ...

maybe_key -> '"' qstr | ident | '"' qstr qstr_scalar | ident noident0 scalar
qstr_scalar -> '"' qstr | '"' qstr scalar
noident0 ::= any printable character that isn't ALPHANUM or '_', '-', '.' (ident0)

hmm, end of scalar is really delimeted by <hsp '\n'>...

S -> hsp(n) value(n)
value(n) -> scalar | key hsp ':' hsp fields(n) | '-' hsp+(n0) elems(n, n0)
scalar -> '\n' | '"' qstr hsp '\n' | number hsp '\n' | printable_no_nl_colon hsp '\n'
fields(n) -> fieldvalue(n) | fieldvalue(n) n key hsp ':' hsp fields(n)
fieldvalue(n) -> scalar | '\n' n hsp+(n0) fields(n n0) | '\n' n hsp(n0) '-' hsp+(n1) elems(n n0, n1)
elems(n, n0) -> value(n ' ' n0) | value(n ' ' n0) n '-' n0 elems(n, n0)

key -> '"' qstr | ident

hsp+(' ' n) -> ' ' hsp(n)
hsp+('\t' n) -> '\t' hsp(n)
hsp() -> eps
hsp(' ' n) -> ' ' hsp(n)
hsp('\t' n) -> '\t' hsp(n)

number -> number0 | number0 number
number0 -> NUMERIC | '_'
printable_no_nl_colon -> PRINTABLE_NO_NL_COLON | PRINTABLE_NO_NL_COLON printable_no_nl_colon

qstr -> '"' | '\' PRINTABLE qstr | PRINTABLE_NO_DQ qstr
ident -> ident0 | ident0 ident
ident0 -> ALPHANUM | '_' | '-' | '.'

PRINTABLE ::= any printable character
PRINTABLE_NO_DQ ::= any printable character except '"'
PRINTABLE_NO_NL_COLON ::= any printable character except '\n', ':'
ALPHANUM ::= any alpha numeric character
NUMERIC ::= any numeric character


rewriting some rules to avoid backtracking:

value(n) -> scalar | key hsp ':' hsp fields(n) | '-' hsp+(n0) elems(n, n0)
fieldvalue(n) -> scalar | '\n' n hsp+(n0) fields(n n0) | '\n' n hsp(n0) '-' hsp+(n1) elems(n n0, n1)
scalar -> '"' qstr hsp '\n' | number hsp '\n' | printable_no_nl_colon hsp '\n'
key -> '"' qstr | ident
is

value(n) -> 
| '-' hsp+(n0) elems(n, n0)   --> array
| '"' qstr hsp '\n'              --> scalar
| '"' qstr hsp ':' hsp fields(n) --> object
  number <= ident <= printable_no_nl_colon
| number hsp '\n'                --> scalar
| ident hsp ':' hsp fields(n)    --> object
| printable_no_nl_colon hsp '\n' --> scalar
fieldvalue(n) ->
| '\n' n hsp+(n0) fields(n n0)   --> object
| '\n' n hsp(n0) '-' hsp+(n1) elems(n n0, n1) --> array
| '"' qstr hsp '\n'              --> scalar
| number hsp '\n'                --> scalar
| printable_no_nl_colon hsp '\n' --> scalar


Going to rewrite it with indent tokens instead of tracking indent directly
ind -> indent peek(not [ws, '-'])
+inda -> '-' hsp+(i) { indent ++= ' ' + i }
+ind -> hsp+(i) { indent ++= i }
-ind -> { [i0, i1, ... in-1, in] = indent } [i0, i1, ...in-1] peek(not ws) { indent-- }


S -> ind(n) value(n)
value(n) -> scalar | key hsp ':' hsp fields(n) | '-' hsp+(i) elems(n, i)
scalar -> '\n' | '"' qstr hsp '\n' | number hsp '\n' | printable_no_nl_colon hsp '\n'
fields(n) -> fieldvalue(n) | fieldvalue(n) ind(n) key hsp ':' hsp fields(n)
fieldvalue(n) -> scalar | '\n' ind(n+1) fields(n+1) | '\n' ind(n+1) '-' hsp+(i) elems(n+1, i)
elems(n, i) : ind += ' ' i -> value(n+1) | value(n+1) n '-' n0 elems(n, n0)

key -> '"' qstr | ident

hsp+(' ' n) -> ' ' hsp(n)
hsp+('\t' n) -> '\t' hsp(n)
hsp() -> eps
hsp(' ' n) -> ' ' hsp(n)
hsp('\t' n) -> '\t' hsp(n)

number -> number0 | number0 number
number0 -> NUMERIC | '_'
printable_no_nl_colon -> PRINTABLE_NO_NL_COLON | PRINTABLE_NO_NL_COLON printable_no_nl_colon

qstr -> '"' | '\' PRINTABLE qstr | PRINTABLE_NO_DQ qstr
ident -> ident0 | ident0 ident
ident0 -> ALPHANUM | '_' | '-' | '.'

PRINTABLE ::= any printable character
PRINTABLE_NO_DQ ::= any printable character except '"'
PRINTABLE_NO_NL_COLON ::= any printable character except '\n', ':'
ALPHANUM ::= any alpha numeric character
NUMERIC ::= any numeric character

*/

typedef struct str_list { char* v; struct str_list* next; } str_list;


typedef struct parser {
	char c;
	int  row, col, n;
	char* line;
	int   linez;
	char* indent;
	int   indentn, indentz;
	char (*getc)(struct parser*);
	int  (*next)(struct parser*);
	int  (*err)(struct parser*, char*, ...);
	void* errdata;
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
	static char cmsg[] = "unexpected char '_', expected '_'";
	static char tmsg[] = "unexpected ind ___, expected '_'";
	if (P->c != c) {
		if (P->c > 0) {
			cmsg[17] = P->c;
			cmsg[31] = c;
			P->err(P, cmsg);
			return 1;
		} else {
			unsigned char i0, i1, i2;
			i0 = (-c)/100;
			i1 = ((-c)/10) - i0 * 10;
			i2 = ((-c) - i1 * 10) - i0 * 100;
			tmsg[15] = i0 ? ' ' : i0 + '0';
			tmsg[16] = i1 ? ' ' : i1 + '0';
			tmsg[17] = i2 + '0';
			tmsg[30] = c;
			P->err(P, tmsg);
			return 1;
		}
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

int par_value(parser*, value*, str_list);
int
par_elems(parser* P, array* a, str_list ind, char* ind0) {
	int err;
#define DO(X) if (err = (X)) return err;
	a->vs = malloc(sizeof(value) * 4);
	str_list indv = (str_list){ .v = ind0, .next = { .v = " ", .next = ind };
	DO(par_value(P, a->vs, indv));
	a->n = 1;
	for (;;) {
		
	}
#undef DO
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

