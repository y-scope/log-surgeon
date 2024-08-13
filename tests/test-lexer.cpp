#include <unistd.h>

#include <catch2/catch_test_macros.hpp>

#include "../src/log_surgeon/Schema.hpp"

TEST_CASE("Schema", "[Schema]") {
    log_surgeon::Schema schema;

    SECTION("Add int") {
        schema.add_variable("myNumber", "123", -1);
        auto const schema_ast = schema.release_schema_ast_ptr();
        REQUIRE(schema_ast->m_schema_vars.size() == 1);
        auto* schemaVarAST
                = dynamic_cast<log_surgeon::SchemaVarAST*>(schema_ast->m_schema_vars[0].get());
        REQUIRE(nullptr != schemaVarAST);
        REQUIRE(schemaVarAST->m_name == "myNumber");
        auto* regexASTCat = dynamic_cast<log_surgeon::finite_automata::RegexASTCat<
                log_surgeon::finite_automata::RegexNFAByteState>*>(schemaVarAST->m_regex_ptr.get());
        REQUIRE(nullptr != regexASTCat);
        REQUIRE(schema.release_schema_ast_ptr()->m_schema_vars.empty());
    }
}
