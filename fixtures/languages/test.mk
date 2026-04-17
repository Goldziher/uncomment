# This comment should be removed

.PHONY: build test clean

# TODO: add install target
build:
	echo "Building # project"

test: build
	echo "Running tests"

clean:
	rm -rf target
