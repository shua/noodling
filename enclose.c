#include <stdio.h>

static unsigned int
code_to_utf8(unsigned char *const buffer, const unsigned int code) {
	if (code <= 0x7F) {
		buffer[0] = code;
		return 1;
	}
	if (code <= 0x7FF) {
		buffer[0] = 0xC0 | (code >> 6);            /* 110xxxxx */
		buffer[1] = 0x80 | (code & 0x3F);          /* 10xxxxxx */
		return 2;
	}
	if (code <= 0xFFFF) {
		buffer[0] = 0xE0 | (code >> 12);           /* 1110xxxx */
		buffer[1] = 0x80 | ((code >> 6) & 0x3F);   /* 10xxxxxx */
		buffer[2] = 0x80 | (code & 0x3F);          /* 10xxxxxx */
		return 3;
	}
	if (code <= 0x10FFFF) {
		buffer[0] = 0xF0 | (code >> 18);           /* 11110xxx */
		buffer[1] = 0x80 | ((code >> 12) & 0x3F);  /* 10xxxxxx */
		buffer[2] = 0x80 | ((code >> 6) & 0x3F);   /* 10xxxxxx */
		buffer[3] = 0x80 | (code & 0x3F);          /* 10xxxxxx */
		return 4;
	}
	return 0;
}

int
main(int argc, char **argv) {
	unsigned char c[4];
	int d;
	unsigned int uz;
	while(read(0, &c[0], 1) == 1) {
		if (c[0] >= 'a' && c[0] <= 'z') {
			d = - 'a' + 0x24D0;
		} else if (c[0] >= 'A' && c[0] <= 'Z') {
			d = -'A' + 0x24B6;
		} else {
			d = 0;
		}
		uz = code_to_utf8(c, d + c[0]);
		if (uz > 0) printf("%c", c[0]);
		if (uz > 1) printf("%c", c[1]);
		if (uz > 2) printf("%c", c[2]);
		if (uz > 3) printf("%c", c[3]);
	}
}
