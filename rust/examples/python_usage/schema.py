#!/usr/bin/env python3

from logmech import ReaderParser
from pattern import PATTERN

parser = ReaderParser()

# Custom delimiters can be specified, though the default is usually sufficient
parser.set_delimiters(" \t\r\n:,!;%@/()[].")

# Step 1 - Timestamp pattern: Extract full HH:MM:SS as components
parser.add_variable_pattern("TIMESTAMP", rf"(?<hour>\d{{2}}):(?<minute>\d{{2}}):(?<second>\d{{2}})")

# Step 2 - Verbosity level: INFO, WARN, ERROR
parser.add_variable_pattern("LEVEL", rf"(?<level>(INFO)|(WARN)|(ERROR))")

# Step 3 - Java exception pattern
parser.add_variable_pattern(
	"SYSTEM_EXCEPTION",
	rf"(?<system_exception_type>({PATTERN.JAVA_PACKAGE_SEGMENT})+[{PATTERN.JAVA_IDENTIFIER_CHARSET}]*Exception): "
	rf"(?<system_exception_msg>{PATTERN.LOG_LINE})"
)

# Stack trace patterns - simplified to avoid issues
parser.add_variable_pattern(
	"SYSTEM_STACK_TRACE",
	rf"\s+at (?<class_method>[a-zA-Z0-9_$\.]+)\((?<source_file>[a-zA-Z0-9_]+\.(java|kt|scala)):(?<line_num>\d+)\)"
)

# Stack trace with jar info
parser.add_variable_pattern(
	"STACK_WITH_JAR",
	rf"\s+at (?<class_method>[a-zA-Z0-9_$\.]+)\((?<source_file>[a-zA-Z0-9_]+\.java):(?<line_num>\d+)\) ~?\[(?<jar>[^\]]+\.jar):(?<version>[^\]]+)\]"
)

# Stack trace with na: prefix
parser.add_variable_pattern(
	"STACK_WITH_NA",
	rf"\s+at (?<class_method>[a-zA-Z0-9_$\.]+)\((?<source_file>[a-zA-Z0-9_]+\.java):(?<line_num>\d+)\) ~?\[na:(?<version>\d+\.\d+[\._]\d+)\]"
)

# Cassandra-specific patterns
parser.add_variable_pattern("STREAM_ID", rf"Stream #(?<stream_id>{PATTERN.UUID})")
parser.add_variable_pattern("HINT_FILE", rf"(?<hint_file>{PATTERN.UUID}\-\d+\-\d+\.hints)")
parser.add_variable_pattern("KEYSPACE_TABLE", rf"Initializing (?<keyspace>[a-z0-9_]+)\.(?<table>[a-z0-9_]+)")
parser.add_variable_pattern("CASSANDRA_HOST", rf"cassandra\-(?<hostname>[a-z0-9\-]+)")
parser.add_variable_pattern("BOOTSTRAP_TOKEN", rf"tokens \[(?<tokens>\-?\d+(, \-?\d+)*)\]")

# Memory patterns
parser.add_variable_pattern("MEMORY_MB", rf"(?<memory>{PATTERN.INT})MB")
parser.add_variable_pattern("MEMORY_BYTES", rf"(?<bytes>\d+)\((?<kb>\d+)K\)")
parser.add_variable_pattern("MEMORY_TYPE", rf"Global memtable (?<memtype>(on\-heap)|(off\-heap)) threshold")

# Duration
parser.add_variable_pattern("DURATION_MS", rf"(?<duration>{PATTERN.INT})\s*ms")

# Netty channel patterns
parser.add_variable_pattern("NETTY_CHANNEL_FULL", rf"channel = \[id: (?<channel_id>0x[a-f0-9]+), /(?<src_ip>{PATTERN.IPV4}):(?<src_port>{PATTERN.PORT}) =\> /(?<dst_ip>{PATTERN.IPV4}):(?<dst_port>{PATTERN.PORT})\]")
parser.add_variable_pattern("NETTY_CHANNEL_SHORT", rf"channel = \[id: (?<channel_id>0x[a-f0-9]+), L:/(?<local_ip>{PATTERN.IPV4})")

# IP patterns
parser.add_variable_pattern("HANDSHAKING_IP", rf"Handshaking version with (?<hostname>[\w\-]+)/(?<ip>{PATTERN.IPV4})")
parser.add_variable_pattern("SESSION_WITH_IP", rf"Session with /(?<ip>{PATTERN.IPV4})")
parser.add_variable_pattern("STREAMING_TO_IP", rf"streaming to /(?<ip>{PATTERN.IPV4})")
parser.add_variable_pattern("CQL_LISTENING", rf"Starting listening for CQL clients on /(?<ip>{PATTERN.IPV4}):(?<port>{PATTERN.PORT})")

# Hinted handoff
parser.add_variable_pattern("HANDOFF_FINISHED", rf"Finished hinted handoff of file (?<file>{PATTERN.UUID}\-\d+\-\d+\.hints) to endpoint /(?<ip>{PATTERN.IPV4}): (?<uuid>{PATTERN.UUID})")

# Directory paths
parser.add_variable_pattern("CASSANDRA_DIR", rf"Directory /var/lib/cassandra/(?<dirtype>(commitlog)|(data)|(saved_caches)|(hints))")

# CompilerOracle patterns
parser.add_variable_pattern("ORACLE_METHOD", rf"CompilerOracle: (?<action>(inline)|(dontinline)) (?<classname>[a-zA-Z0-9_/$]+)\.(?<method>\w+)")
parser.add_variable_pattern("ORACLE_VARIANT_METHOD", rf"org/apache/cassandra/db/transform/StoppingTransformation\.(?<stopmethod>(stop)|(stopInPartition))")
parser.add_variable_pattern("MEMORY_CLASS_METHOD", rf"org/apache/cassandra/io/util/(?<memclass>(Memory)|(SafeMemory))\.checkBounds")

# Status patterns
parser.add_variable_pattern("JOINING_STATUS", rf"JOINING: (?<status>(schema)|(calculation)) complete")

# Thread names
parser.add_variable_pattern("THREAD_NAME", rf"Thread\[(?<thread>[^\]]+)\]")

# Unknown column
parser.add_variable_pattern("UNKNOWN_COLUMN", rf"Unknown column (?<column>\w+)")

# Frames omitted
parser.add_variable_pattern("FRAMES_OMITTED", rf"\.\.\. (?<frames>\d+) common frames omitted")

# General paths
parser.add_variable_pattern("PATH", rf"(?<path>/[\w/\-\.]+)")
parser.add_variable_pattern("FILE_PATH", rf"(?<filepath>[^\s]+\.(jar|properties|yaml|log))")

# Class names
parser.add_variable_pattern("CLASS_NAME", rf"(?<classname>[a-zA-Z0-9_$]+)")

# Long numbers (7-20 digits)
parser.add_variable_pattern("LONG_NUMBER", rf"(?<long>\-?\d{{7,20}})")

# Generic patterns - should be last
parser.add_variable_pattern("SYSTEM_IP", rf"(?<ip>{PATTERN.IPV4})")
parser.add_variable_pattern("SYSTEM_UUID", rf"(?<uuid>{PATTERN.UUID})")
parser.add_variable_pattern("GENERIC_FLOAT", rf"(?<float>{PATTERN.FLOAT})")
parser.add_variable_pattern("GENERIC_INT", rf"(?<int>{PATTERN.INT})")
parser.add_variable_pattern("HEX_NUMBER", rf"(?<hex>0x[a-f0-9]+)")
parser.add_variable_pattern("PORT_NUMBER", rf"(?<port>{PATTERN.PORT})")

parser.compile()
