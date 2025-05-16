# Examples

The first two example programs in this directory are `buffer-parser` and
`reader-parser` corresponding to the [two API styles][1]. They demonstrate
parsing a log file and printing out the timestamp and log-level of each message,
as well as any multiline log messages.

The third example is `intersect-test` which demonstrates the result of taking
the intersection between a schema DFA and a search query DFA.

## Building

First, ensure you've built and installed the library by following
[these steps][2]. Then run the following command:

```shell
task build:examples-debug
```

## Running

The example programs can be run as follows:

```shell
./build/examples/debug/buffer-parser ./examples/schema.txt log.txt
./build/examples/debug/reader-parser ./examples/schema.txt log.txt
./build/examples/debug/intersect-test
```

where:
* `./examples/schema.txt` is a schema file containing rules for variables that 
  should be parsed.
* `log.txt` is a log file.

[1]: ../docs/design-objectives.md#api-styles
[2]: ../README.md#building-and-installing
