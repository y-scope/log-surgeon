#include "log_surgeon/log_surgeon.hpp"

#include <cassert>
#include <cstdio>
#include <iostream>
#include <optional>
#include <string_view>

using namespace log_surgeon;

int main() {
    Box<Schema> schema{log_surgeon_schema_new()};

    log_surgeon_schema_add_rule_with_priority(schema, 0, "hello"_rust, "abc|d(?<foo>[a-z])f"_rust);

    ParserHandle parser{schema};

    CArray<char> const input{"def foobarbaz"_rust};
    size_t pos{0};

    std::optional<EventHandle> maybe_event{parser.next_event(input, &pos)};
    assert(maybe_event.has_value());
    assert(pos == input.length);

    EventHandle event{*maybe_event};
    // assert(event.log_type() == "%hello% foobarbaz");
    assert(event.log_type() == "d%1.1:hello.foo%f foobarbaz");

    std::optional<CCapture> maybe_capture{event.get_capture(0)};
    assert(maybe_capture.has_value());

    CCapture const& cap{*maybe_capture};
    assert(cap.variable_name.as_cpp_view() == "hello");

    assert(event.get_variable_window(0).has_value());
    assert(event.get_variable_window(0) == std::make_optional(std::make_pair(0, 3)));

    assert(!event.get_capture(1).has_value());

    printf("good!\n");

    log_surgeon_schema_drop(schema);

    return 0;
}
