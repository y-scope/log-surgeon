#!/usr/bin/env python3

import unittest
from textwrap import dedent

from log_surgeon import Parser

class TestSimple(unittest.TestCase):
	def setUp(self):
		pass

	def test_basic(self):
		p = Parser(debug=True)

		p.add_variable_pattern("number", r"[0-9]+")
		p.add_variable_pattern("at_host", r"@(?<inside>[a-z]+)(?<parts>(?<dot>\.)[a-z]*(?<end>[a-z]))*")

		p.compile()

		text = dedent("""
		123 qwerty 4567 @example someone@example @example.foo.bar.baz
		""")

		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertIsNotNone(event)
		self.assertEqual(str(event.log_type), "\n")

		event = p.next_log_event()
		self.assertIsNotNone(event)
		parts = [
			"%1.0:number.%",
			" qwerty ",
			"%1.0:number.%",
			" @",
			"%2.1:at_host.inside%",
			" someone@example @",
			"%2.1:at_host.inside%",
			"%2.3:at_host.dot%",
			"fo",
			"%2.4:at_host.end%",
			"%2.3:at_host.dot%",
			"ba",
			"%2.4:at_host.end%",
			"%2.3:at_host.dot%",
			"ba",
			"%2.4:at_host.end%",
			"\n",
		]
		self.assertEqual(str(event.log_type), ''.join(parts))

		self.assertIsNone(p.next_log_event())

	def test_anchors(self):
		p = Parser(debug=True)

		p.set_delimiters(" ")
		p.add_variable_pattern("word", r"[a-z]+")
		p.add_variable_pattern("int1", r"^\d+")
		p.add_variable_pattern("int2", r"\d+$")

		p.compile()

		text = "abc123"
		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "%1.0:word.%%3.0:int2.%")

		text = "abc 123"
		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "%1.0:word.% %2.0:int1.%")

		text = "123abc"
		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "%2.0:int1.%%1.0:word.%")

		text = "abc123abc"
		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "%1.0:word.%123abc")

		text = "abc123 abc"
		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "%1.0:word.%%3.0:int2.% %1.0:word.%")

	def test_log_type_eq(self):
		p = Parser(debug=True)

		p.set_delimiters(" ")
		p.add_variable_pattern("word", r"[a-z]+")

		p.compile()

		text = dedent("""\
		line 1
		line 2
		""")

		p.set_input_stream(text)
		e1 = p.next_log_event()

		self.assertEqual(e1.message, "line 1\n")

		p.set_input_stream(text)
		e2 = p.next_log_event()
		e3 = p.next_log_event()

		# `LogEvent` doesn't implement `__eq__`.
		self.assertNotEqual(e1, e2)

		self.assertEqual(e1.log_type, e2.log_type)
		self.assertNotEqual(e2.log_type, e3.log_type)

	def test_priority(self):
		p = Parser(debug=True)

		p.set_delimiters(" ")

		# Earlier patterns have priority;
		# if both the first and second rules match (with the same length),
		# `"var1"` will be returned.
		# If both the second and third rules match (with the same length),
		# `"var2"` will be returned.
		p.add_variable_pattern("var1", r"[a-z]+")
		p.add_variable_pattern("var2", r"[a-z0-9]+")
		p.add_variable_pattern("var1", r"[0-9]+")

		# Add them in reverse order, but hardcode the priority
		# (so that the end result should be as above).
		p.add_variable_pattern("var1", r":[0-9]+", priority=10)
		p.add_variable_pattern("var2", r":[a-z0-9]+", priority=20)
		p.add_variable_pattern("var1", r":[a-z]+", priority=30)

		p.compile()

		text = dedent("""\
		abc
		123
		:abc
		:123
		""")

		p.set_input_stream(text)

		e1 = p.next_log_event()
		self.assertEqual(str(e1.log_type), "%4.0:var1.%\n")

		e2 = p.next_log_event()
		self.assertEqual(str(e2.log_type), "%5.0:var2.%\n")

		e3 = p.next_log_event()
		self.assertEqual(str(e3.log_type), "%1.0:var1.%\n")

		e4 = p.next_log_event()
		self.assertEqual(str(e4.log_type), "%2.0:var2.%\n")

	def test_variable_offsets(self):
		p = Parser(debug=True)

		p.set_delimiters(" ")
		p.add_variable_pattern("int", r"[0-9]+")

		p.compile()

		text = "0 234  789"

		p.set_input_stream(text)
		e = p.next_log_event()

		self.assertEqual(e.message, text)
		self.assertEqual(len(e.leaf_captures), 3)

		self.assertEqual(e.leaf_captures[2].offsets.start, 7)
		self.assertEqual(e.leaf_captures[2].offsets.stop, 10)

		for cap in e.leaf_captures:
			self.assertEqual(cap.text, e.message[cap.offsets])

	def test_nested_captures(self):
		p = Parser(debug=True)

		p.set_delimiters(" ")
		p.add_variable_pattern("wordint", r":(?<word>[a-z]+(?<int>[0-9]+))")

		p.compile()

		line = ":abc123\n"
		N = 3

		p.set_input_stream(line * N)

		for i in range(N):
			e = p.next_log_event()

			self.assertEqual(e.message, line)
			self.assertEqual(len(e.leaf_captures), 1)
			self.assertEqual(e.leaf_captures[0].offsets, slice(len(":abc"), len(line) - 1, 1))
			self.assertEqual(e.leaf_captures[0].text, "123")

			self.assertEqual(len(e.non_leaf_captures), 2)
			self.assertEqual(e.non_leaf_captures[0].offsets, slice(0, len(line) - 1, 1))
			self.assertEqual(e.non_leaf_captures[0].text, ":abc123")
			self.assertEqual(e.non_leaf_captures[1].offsets, slice(len(":"), len(line) - 1, 1))
			self.assertEqual(e.non_leaf_captures[1].text, "abc123")

			self.assertEqual(len(e.variables), 1)
			self.assertEqual(e.variables[0].offsets, slice(0, len(line) - 1, 1))
			self.assertEqual(e.variables[0].variable_name, "wordint")
			self.assertEqual(e.variables[0].text, ":abc123")

	def test_headers(self):
		p = Parser(debug=True)

		p.set_delimiters(" ")
		p.add_variable_pattern("header", r"\d{4}\-\d{2}\-\d{2} \d{2}")
		p.add_variable_pattern("word", r"\w+")

		p.compile()

		text = dedent("""\
		1234-56-78 00 one two three four
		1234-56-78 01 five
		1234-56-78 02 six seven
		""")

		p.set_input_stream(text)

		e = p.next_log_event()
		self.assertEqual(len(e.variables), 5)

		e = p.next_log_event()
		self.assertEqual(len(e.variables), 2)

		e = p.next_log_event()
		self.assertEqual(len(e.variables), 3)

		e = p.next_log_event()
		self.assertIsNone(e)

	def test_parent_captures(self):
		p = Parser(debug=True)

		p.set_delimiters(" ")
		p.add_variable_pattern("zero", r"0(?<one>1(?<two>2(?<three>3)))")

		p.compile()

		text = "0123"

		p.set_input_stream(text)

		e = p.next_log_event()

		self.assertEqual(len(e.variables), 1)
		self.assertEqual(len(e.leaf_captures), 1)
		self.assertEqual(len(e.all_captures), 4)

		leaf = e.leaf_captures[0]

		self.assertEqual(leaf.name, "three")
		self.assertEqual(leaf.text, "3")

		self.assertEqual(leaf.parent.name, "two")
		self.assertEqual(leaf.parent.text, "23")

		self.assertEqual(leaf.parent.parent.name, "one")
		self.assertEqual(leaf.parent.parent.text, "123")

		self.assertEqual(leaf.parent.parent.parent.name, "zero")
		self.assertEqual(leaf.parent.parent.parent.text, "0123")

		self.assertIsNone(leaf.parent.parent.parent.parent)
