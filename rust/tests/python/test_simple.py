#!/usr/bin/env python3

import unittest
from textwrap import dedent

from logmech import ReaderParser

class TestSimple(unittest.TestCase):
	def setUp(self):
		pass

	def test1(self):
		p = ReaderParser()

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

	def test2(self):
		p = ReaderParser()

		p.set_delimiters(" \t\r\n:,!;%@/()[].=")
		p.add_variable_pattern("handler_class", r"for class (?<handler_class>org\.apache\.hadoop\.yarn\.server\.[a-zA-Z0-9\.$]+)")
		p.add_variable_pattern("container", r"container[0-9_]+")

		p.compile()

		text = "Starting resource-monitoring for container_1427088391284_0021_01_000024"

		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "Starting resource-monitoring for %container%")

		self.assertIsNone(p.next_log_event())

	def test3(self):
		p = ReaderParser()

		p.set_delimiters(r" \t\r\n,!;%@=()\[\]")
		p.add_variable_pattern("c", r"Container")
		p.add_variable_pattern("VAR", r"[a-zA-Z0-9_\.\-/\\#!]*[0-9][a-zA-Z0-9_\.\-/\\]*")

		p.compile()

		text = "INFO [ContainerLauncher #32145]"

		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "INFO [%c%Launcher %VAR%]")
