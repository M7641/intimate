# macOS ARM64 build (Apple Silicon).
# clang drives both the assembler and the linker — it figures out
# the SDK path, -platform_version, and -lSystem on its own.
#
# The original Linux x86-64 source is preserved in server.asm for reference.

default: build

build:
	clang -arch arm64 -o server server.s

run: build
	./server

clean:
	rm -f *.o server
