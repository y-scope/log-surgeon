#!/usr/bin/env python3

import unittest
from textwrap import dedent

from log_surgeon import Parser

class TestSimple(unittest.TestCase):
	def setUp(self):
		pass

	def test1(self):
		p = Parser()

		p.set_delimiters(" \t\r\n:,!;%@/()[].=")
		p.add_variable_pattern("handler_class", r"for class (?<handler_class>org\.apache\.hadoop\.yarn\.server\.[a-zA-Z0-9\.\$]+)")
		p.add_variable_pattern("container", r"container[0-9_]+")

		p.compile()

		text = "Starting resource-monitoring for container_1427088391284_0021_01_000024"

		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "Starting resource-monitoring for %container%")

		self.assertIsNone(p.next_log_event())

	def test2(self):
		p = Parser()

		p.set_delimiters(" \t\r\n,!;%@=()[]")
		p.add_variable_pattern("c", r"Container")
		p.add_variable_pattern("VAR", r"[a-zA-Z0-9_\.\-/\\#!]*[0-9][a-zA-Z0-9_\.\-/\\]*")

		p.compile()

		text = "INFO [ContainerLauncher #32145]"

		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "INFO [%c%Launcher %VAR%]")

	def test3(self):
		p = Parser()

		p.set_delimiters(" \t\r\n!\"#\\$%&'()*,:;<=>?{}@()[|]^_`~'")
		p.add_variable_pattern("role", r"'roles': \[u'(?<role>[^']+)'\]")

		p.compile()

		text = "'roles': [u'_member_']"

		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), "%role%")

		text = "a'roles': [u'_member_']"

		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), text)

		text = " 'roles': [u'_member_']"

		p.set_input_stream(text)

		event = p.next_log_event()
		self.assertEqual(str(event.log_type), " %role%")
