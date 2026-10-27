CXX = clang++

usage.o: usage.cpp ../../target/debug/liblog_surgeon.a
	$(CXX) -std=c++20 -g \
		-I../../cxx \
		-o $@ $^
