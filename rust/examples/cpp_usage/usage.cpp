#include "log_surgeon/generated_bindings.hpp"
#include "log_surgeon/log_surgeon.hpp"

#include <cassert>
#include <cstdio>
#include <iostream>
#include <optional>
#include <span>
#include <string_view>

using namespace log_surgeon;

static void try_search();
static void try_interpretations();

int main() {
    Box<SchemaBuilder> builder{log_surgeon_schema_builder_new()};

    log_surgeon_schema_builder_add_rule_with_priority(builder, 0, "hello"_rust, "abc|d(?<foo>[a-z])f"_rust);

    ParserHandle parser{log_surgeon_schema_builder_build(builder)};

    CArray<char> const input{"def foobarbaz"_rust};
    size_t pos{0};

    std::optional<EventHandle> maybe_event{parser.next_event(input, &pos)};
    assert(maybe_event.has_value());
    assert(pos == input.length);

    EventHandle event{*maybe_event};
    // assert(event.log_type() == "%hello% foobarbaz");
    assert(event.log_type() == "d%1.1:hello.foo%f foobarbaz");

    std::optional<Match> maybe_match{event.get_leaf_match(0)};
    assert(maybe_match.has_value());

    Match const& mat{*maybe_match};
    assert(mat.ffi_pointers.rule_name.as_cpp_view() == "hello");

    assert(!event.get_leaf_match(1).has_value());

    assert(event.get_all_matches().size() == 2);

    try_search();

    try_interpretations();

    printf("good!\n");

    return 0;
}

static void try_search() {
    Box<SchemaBuilder> builder{log_surgeon_schema_builder_new()};

    log_surgeon_schema_builder_add_rule_with_priority(builder, 0, "foo"_rust, ":::(?<bar>[a-z]+(\\.(?<baz>[0-9]+))*)"_rust);

    Box<Schema> schema{log_surgeon_schema_builder_build(builder)};

    Option<Box<SearchResult>> search{log_surgeon_search_by_named_type(schema, "foo.bar"_rust, "hello.123.456"_rust)};
    assert(search != nullptr);

    size_t len{0};
    Match const* matches{log_surgeon_search_result_get_leaf_matches(search, &len)};
    assert(len == 2);

    assert(matches[0].rule_idx != 0);
    assert(matches[0].ffi_pointers.sub_rule_name.as_cpp_view() == "baz");
    assert(matches[0].ffi_pointers.lexeme.as_cpp_view() == "123");

    assert(matches[1].rule_idx != 0);
    assert(matches[1].ffi_pointers.sub_rule_name.as_cpp_view() == "baz");
    assert(matches[1].ffi_pointers.lexeme.as_cpp_view() == "456");
}

static void try_interpretations() {
    Box<SchemaBuilder> builder{log_surgeon_schema_builder_new()};

    log_surgeon_schema_builder_add_rule_with_priority(builder, 0, "email"_rust, R"((?<user>\w+)@((?<parts>\w+)\.)+(?<tld>\w+))"_rust);

    ParserHandle parser{log_surgeon_schema_builder_build(builder)};

    std::vector<std::vector<SubQuery>> interpretations{parser.query_interpretations("email"_rust, "a*@*com"_rust)};

    std::cout << "== Interpretations" << std::endl;
    for (std::vector<SubQuery> const& sub_queries : interpretations) {
        std::cout << "- ";
        for (SubQuery const& sub_query : sub_queries) {
            if (sub_query.rule_idx == 0) {
                std::cout << sub_query.value;
            } else {
                std::cout << "(?<" << sub_query.qualified_name << ">" << sub_query.value << ")";
            }
        }
        std::cout << std::endl;
    }
}
