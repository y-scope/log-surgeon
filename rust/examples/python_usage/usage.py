#!/usr/bin/env python3

from logmech import ReaderParser

a = ReaderParser()

a.add_variable_pattern("number", r"[0-9]+")
a.add_variable_pattern("username", r"@(?<inside>[a-z]+)(?<parts>\.[a-z]+)*")

a.compile()

text = """
123 awesrgesrgesrg 6346346 @someone foo@username @someone.foo.bar.baz
foo
bar
baz
"""

a.set_input_stream(text)

print(f"py: input [{text}]")
print()

while True:
	event = a.next_log_event()

	if event is None:
		break

	print(f"header is: '{event.header!r}'")
	print(f"tokens are:")
	for t in event.tokens:
		# print(f"- ({t!r}): {t.rule}, '{t.text}', {t.captures}")
		print(f"- {t.rule}, '{t.text}'")

	print()
