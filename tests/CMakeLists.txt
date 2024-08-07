set(SOURCE_FILES_unitTest
    src/log_surgeon/FileReader.cpp
    src/log_surgeon/FileReader.hpp
    src/log_surgeon/finite_automata/RegexAST.hpp
    src/log_surgeon/finite_automata/RegexAST.tpp
    src/log_surgeon/finite_automata/RegexNFA.hpp
    src/log_surgeon/finite_automata/RegexNFA.tpp
    src/log_surgeon/LALR1Parser.cpp
    src/log_surgeon/LALR1Parser.hpp
    src/log_surgeon/LALR1Parser.tpp
    src/log_surgeon/ParserInputBuffer.hpp
    src/log_surgeon/ParserInputBuffer.cpp
    src/log_surgeon/Schema.hpp
    src/log_surgeon/Schema.cpp
    src/log_surgeon/SchemaParser.cpp
    src/log_surgeon/SchemaParser.hpp
    src/log_surgeon/Token.cpp
    src/log_surgeon/Token.hpp
    tests/test-lexer.cpp
    tests/test-main.cpp
)

add_executable(unitTest ${SOURCE_FILES_unitTest})
target_include_directories(unitTest
                           PRIVATE
                           ${CMAKE_SOURCE_DIR}/submodules
                           ${CMAKE_SOURCE_DIR}/src
)
target_compile_features(unitTest
                        PRIVATE cxx_std_20
)
