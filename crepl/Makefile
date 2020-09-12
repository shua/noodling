LEX=lex -X
YACC=yacc -y
CC=clang

crepl: c.l c.y crepl.c
	$(LEX) c.l
	$(YACC) -d c.y
	$(CC) -o crepl crepl.c y.tab.c lex.yy.c


.PHONY: clean
clean:
	rm y.tab.c y.tab.h lex.yy.c crepl
