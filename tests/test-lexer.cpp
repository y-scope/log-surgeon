#include <catch2/catch_test_macros.hpp>

#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/finite_automata/RegexNFA.hpp>
#include <log_surgeon/Schema.hpp>
#include <log_surgeon/SchemaParser.hpp>

TEST_CASE("Schema", "[Schema]") {
    SECTION("Add int") {
        log_surgeon::Schema schema;
        schema.add_variable("myNumber", "123", -1);
        auto const schema_ast = schema.release_schema_ast_ptr();
        REQUIRE(schema_ast->m_schema_vars.size() == 1);
        auto* schema_var_ast
                = dynamic_cast<log_surgeon::SchemaVarAST*>(schema_ast->m_schema_vars[0].get());
        REQUIRE(nullptr != schema_var_ast);
        REQUIRE(schema_var_ast->m_name == "myNumber");
        auto* regexASTCat = dynamic_cast<log_surgeon::finite_automata::RegexASTCat<
                log_surgeon::finite_automata::RegexNFAByteState>*>(schema_var_ast->m_regex_ptr.get()
        );
        REQUIRE(nullptr != regexASTCat);
        REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());
    }
}
