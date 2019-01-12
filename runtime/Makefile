CC=cc
CFLAGS=-O3 -Wall -g

ARCHIVE = libruntime.a
HEADERS = lib/ref.h lib/object.h lib/array.h lib/string.h lib/extern.h lib/thread.h lib/monitor.h lib/utils.h
OBJECTS = api/start.o api/object.o api/array.o api/string.o api/throw.o api/native.o api/stubs.o

$(ARCHIVE): $(OBJECTS)
	ar ru $@ $^
	ranlib $@

%.o: %.c $(HEADERS)
	$(CC) -c -o $@ $< $(CFLAGS)

.PHONY: clean
clean:
	rm -f $(ARCHIVE) $(OBJECTS)
