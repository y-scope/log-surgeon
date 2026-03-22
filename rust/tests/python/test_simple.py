#!/usr/bin/env python3

import unittest
from textwrap import dedent

from log_surgeon import Parser

class TestSimple(unittest.TestCase):
	def setUp(self):
		pass

	def test_basic(self):
		p = Parser()

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
		self.assertEqual(str(event.log_type), "%number% qwerty %number% %at_host% someone@example %at_host%\n")
		self.assertEqual(event.variables[0].name, "number")
		self.assertEqual(event.variables[0].text, "123")
		self.assertEqual(event.variables[1].name, "number")
		self.assertEqual(event.variables[1].text, "4567")
		self.assertEqual(event.variables[2].name, "at_host")
		self.assertEqual(event.variables[2].text, "@example")
		self.assertEqual(event.variables[3].name, "at_host")
		self.assertEqual(event.variables[3].text, "@example.foo.bar.baz")
		self.assertEqual(event.variables[3].captures["dot"], ["."] * 3)
		self.assertEqual(event.variables[3].captures["end"], ["o", "r", "z"])
		self.assertEqual(event.variables[3].captures["inside"], ["example"])
		self.assertEqual(event.variables[3].captures["parts"], [".foo", ".bar", ".baz"])

		self.assertIsNone(p.next_log_event())

	def test_anchors(self):
		p = Parser()

		p.set_delimiters(" ")
		p.add_variable_pattern("word", r"[a-z]+")
		p.add_variable_pattern("int1", r"^\d+")
		p.add_variable_pattern("int2", r"\d+$")

		p.compile()

		text = "abc123"
		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "%word%%int2%")

		text = "abc 123"
		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "%word% %int1%")

		text = "123abc"
		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "%int1%%word%")

		text = "abc123abc"
		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "%word%123abc")

		text = "abc123 abc"
		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "%word%%int2% %word%")

	def test_log_type_eq(self):
		p = Parser()

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
		p = Parser()

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
		self.assertEqual(str(e1.log_type), "%var1%\n")

		e2 = p.next_log_event()
		self.assertEqual(str(e2.log_type), "%var2%\n")

		e3 = p.next_log_event()
		self.assertEqual(str(e3.log_type), "%var1%\n")

		e4 = p.next_log_event()
		self.assertEqual(str(e4.log_type), "%var2%\n")

	def test_variable_offsets(self):
		p = Parser()

		p.set_delimiters(" ")
		p.add_variable_pattern("int", r"[0-9]+")

		p.compile()

		text = "0 234  789"

		p.set_input_stream(text)
		e = p.next_log_event()

		self.assertEqual(e.message, text)
		self.assertEqual(len(e.variables), 3)

		self.assertEqual(e.variables[2].offsets.start, 7)
		self.assertEqual(e.variables[2].offsets.stop, 10)

		for var in e.variables:
			self.assertEqual(var.text, e.message[var.offsets])
