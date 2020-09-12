all: cyamlish-flow cyamlish-indent cyamlish

cyamlish-flow: flow.c
	cc -g $< -o $@ -Wno-parentheses

cyamlish: main.c
	cc -g $< -o $@ -Wno-parentheses

cyamlish-indent: indent.c
	cc -g $< -o $@ -Wno-parentheses
