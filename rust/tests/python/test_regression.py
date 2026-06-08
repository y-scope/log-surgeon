#!/usr/bin/env python3

import unittest
from textwrap import dedent

from log_surgeon import Parser

class TestRegression(unittest.TestCase):
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
		# self.assertEqual(str(event.log_type), "Starting resource-monitoring for %2.0:container.%")

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
		# self.assertEqual(str(event.log_type), "INFO [%1.0:c.%Launcher %2.0:VAR.%]")

	def test3(self):
		p = Parser()

		p.set_delimiters(" \t\r\n!\"#\\$%&'()*,:;<=>?{}@()[|]^_`~'")
		p.add_variable_pattern("role", r"'roles': \[u'(?<role>[^']+)'\]")

		p.compile()

		text = "'roles': [u'_member_']"

		p.set_input_stream(text)

		event = p.next_log_event()
		# self.assertEqual(str(event.log_type), "%1.1:role.role%")
		# self.assertEqual(str(event.log_type), r"'roles': [u'%1.1:role.role%']")

		text = "a'roles': [u'_member_']"

		p.set_input_stream(text)

		event = p.next_log_event()
		# self.assertEqual(str(event.log_type), text)

		text = " 'roles': [u'_member_']"

		p.set_input_stream(text)

		event = p.next_log_event()
		# self.assertEqual(str(event.log_type), " %role%")
		# self.assertEqual(str(event.log_type), " 'roles': [u'%1.1:role.role%']")

	def test_headers(self):
		p = Parser()

		p.set_delimiters(" \t\r\n")
		p.add_variable_pattern("header", r"\d{4}\-\d{2}\-\d{2} \d{2}:\d{2}:\d{2},\d{3}")

		p.compile()

		text = dedent("""\
		2018-06-20 00:00:09,601 DEBUG First event
		2018-06-20 00:00:09,602 DEBUG Second event
		seqno: 19
		lastPacketInBlock: false
		2018-06-20 00:00:09,603 DEBUG Third event
		""")

		p.set_input_stream(text)

		n = 0
		while p.next_log_event() is not None:
			n += 1

		self.assertEqual(n, 3)

	def test_all_anchors(self):
		p = Parser()

		p.set_delimiters(" \t\r\n")
		p.add_variable_pattern("num", r"^[0-9]+$")
		p.add_variable_pattern("word", r"^[a-z]+$")

		p.compile()

		text = "abc 123 def"

		p.set_input_stream(text)

		e = p.next_log_event()
		self.assertIsNotNone(e)

		self.assertEqual(len(e.variables), 3)
		self.assertEqual(e.variables[0].text, "abc")
		self.assertEqual(e.variables[1].text, "123")
		self.assertEqual(e.variables[2].text, "def")
