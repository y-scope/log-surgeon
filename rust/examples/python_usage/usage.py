#!/usr/bin/env python3

from logmech import ReaderParser

a = ReaderParser()
a.add_variable_pattern("hello", "abc|d(?<foo>[a-z])f")
text = "def foobarbaz"

a.compile()

a.set_input_stream(text)

print(f"py: input [{text}]")
print()

while True:
	event = a.next_log_event()

	if event is None:
		break

	# print(f"header is: '{event.header!r}'")
	print(f"log type is '{event.log_type}'")
	print(f"variables are:")
	for v in event.variables:
		print(f"- {v.name}, '{v.text}'")
		for k, v in v.captures.items():
			print(f"\t- {k}: {v}")

	print()
