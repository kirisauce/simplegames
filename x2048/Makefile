

x2048-cc: 2048.cc
	set -eu; \
	tmp=`pkg-config --cflags --libs ncursesw icu-i18n`; \
	$(CXX) 2048.cc -o x2048-cc $$tmp

clean:
	rm -rf x2048-cc

.PHONY: clean
