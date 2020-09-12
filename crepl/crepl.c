#include <ctype.h>
#include <sys/wait.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include "tag.h"

int yyparse();

char *buf=NULL;
int bufz=1, bufc=0;
void
append(char c) {
	if (bufc+1 >= bufz) buf = realloc(buf, bufz*=2);
	buf[bufc++] = c;
}

int
main(void) {
	int srcpipe[2], tagpipe[2];
	pid_t cpid;
	char b,b0;
	enum tag_e t;

	if (pipe(srcpipe) == -1) return perror("pipe"), 2;
	if (pipe(tagpipe) == -1) return perror("pipe"), 2;
	cpid = fork();
	if (cpid == -1) return perror("fork"), 2;

	if (cpid == 0) {
		close(srcpipe[0]);
		dup2(srcpipe[1], 1); close(srcpipe[1]);

		yyparse();

		close(1);
		return 0;

	} else {
		close(srcpipe[1]);
		dup2(srcpipe[0], 0);
		close(srcpipe[0]);

		while (read(0, &b, 1) > 0) {
			if (b == 0) {
				char *buf0 = buf;
				buf[bufc] = 0;
				while (isspace(buf[0])) buf++;
				switch (b0) {
				case func_tag: printf("function:    %s\n", buf); break;
				case decl_tag: printf("declaration: %s\n", buf); break;
				case stmt_tag: printf("statement:   %s\n", buf); break;
				default:       printf("unknown:     %s\n", buf); break;
				}
				buf = buf0;
				buf = realloc(buf, bufz=1);
				buf[0] = 0;
				b0 = 0;
				bufc = 0;
			} else {
				if (b0) append(b0);
				b0 = b;
			}
		}

		if (buf) free(buf);
		wait(NULL);
		return 0;
	}
}
