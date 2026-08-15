ROOT ?= root

build:
	cargo build --release

install:
	mkdir --parent "$(ROOT)/bin"
	mv --force target/release/runbox "$(ROOT)/bin"

clean:
	rm --recursive --force target
