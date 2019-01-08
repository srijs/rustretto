CC=clang
CFLAGS=-O3 -Wall

ARCHIVE = libruntime.a
HEADERS = ref.h
OBJECTS = api.o native.o stubs.o

$(ARCHIVE): $(OBJECTS)
	ar ru $@ $^
	ranlib $@

%.o: %.c $(HEADERS)
	$(CC) -c -o $@ $< $(CFLAGS)

.PHONY: clean
clean:
	rm -f $(ARCHIVE) $(OBJECTS)
