# Examples

There are two example programs in this directory, `buffer-parser` and
`reader-parser` corresponding to the [two API styles][1]. They demonstrate
parsing a log file and printing out the timestamp and log-level of each message,
as well as any multiline log messages. 

## Building

First, ensure you've built and installed the library by following
[these steps][2]. Then run the following commands (from the repo's root):

```shell
# Generate the CMake project
cmake -S examples -B examples/build
# Build the project
cmake --build examples/build -j
```

## Running

The example programs can be run as follows:

```shell
./examples/build/buffer-parser ./examples/schema.txt log.txt
./examples/build/reader-parser ./examples/schema.txt log.txt
```

where:
* `./examples/schema.txt` is a schema file containing rules for variables that 
  should be parsed.
* `log.txt` is a log file.

[1]: ../docs/design-objectives.md#api-styles
[2]: ../README.md#building-and-installing
