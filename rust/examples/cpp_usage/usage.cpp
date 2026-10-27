#include "log_surgeon/log_surgeon.hpp"

#include <cassert>
#include <cstdio>
#include <iostream>
#include <optional>
#include <span>
#include <string_view>

using namespace log_surgeon;

static void try_interpretations();

int main() {
    ParsingSpecBuilder builder;

    builder.add_rule_with_priority("hello", "abc|d(?<foo>[a-z])f");

    ParsingSpec spec{builder.build()};
    Parser parser{spec.create_parser()};

    CArray<char> const input{"def foobarbaz\n"_rust};
    size_t pos{0};

    std::optional<LogEvent> maybe_event{parser.next_event(input, &pos)};
    assert(maybe_event.has_value());
    assert(pos == input.length);

    LogEvent event{*maybe_event};

    assert(event.get_all_matches().size() == 2);

    {
        Match const& mat{event.get_all_matches()[0]};
        assert(mat.get_rule_name() == "hello");
    }

    {
        std::optional<Match> maybe_match{event.get_leaf_match(0)};
        assert(maybe_match.has_value());

        Match const& mat{*maybe_match};
        assert(mat.get_rule_name() == "foo");
    }

    { assert(!event.get_leaf_match(1).has_value()); }

    try_interpretations();

    printf("good!\n");

    return 0;
}

static void try_interpretations() {
    ParsingSpecBuilder builder;

    builder.add_rule_with_priority("email"_rust, R"((?<user>\w+)@((?<parts>\w+)\.)+(?<tld>\w+))");

    ParsingSpec spec{builder.build()};

    std::vector<std::vector<LeafQuery>> interpretations{
            spec.search_by_name("a*@*com"_rust, "email"_rust)
    };

    std::cout << "== Interpretations" << std::endl;
    for (std::vector<LeafQuery> const& queries : interpretations) {
        std::cout << "- ";
        for (LeafQuery const& leaf_query : queries) {
            if (leaf_query.name.empty()) {
                std::cout << leaf_query.value;
            } else {
                std::cout << "(?<" << leaf_query.name << ">" << leaf_query.value << ")";
            }
        }
        std::cout << std::endl;
    }
}
