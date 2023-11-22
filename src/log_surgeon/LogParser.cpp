#include "LogParser.hpp"

#include <filesystem>
#include <iostream>
#include <memory>
#include <optional>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/FileReader.hpp>
#include <log_surgeon/SchemaParser.hpp>

using std::make_unique;
using std::runtime_error;
using std::string;
using std::to_string;
using std::unique_ptr;
using std::vector;

namespace log_surgeon {
using finite_automata::RegexAST;
using finite_automata::RegexASTCat;
using finite_automata::RegexASTGroup;
using finite_automata::RegexASTInteger;
using finite_automata::RegexASTLiteral;
using finite_automata::RegexASTMultiplication;
using finite_automata::RegexASTOr;
using finite_automata::RegexDFAByteState;
using finite_automata::RegexNFAByteState;

LogParser::LogParser(string const& schema_file_path)
        : LogParser::LogParser(SchemaParser::try_schema_file(schema_file_path).get()) {}

LogParser::LogParser(SchemaAST const* schema_ast) {
    add_rules(schema_ast);
    m_lexer.generate();
    m_log_event_view = make_unique<LogEventView>(*this);
}

auto LogParser::add_delimiters(unique_ptr<ParserAST> const& delimiters) -> void {
    auto* delimiters_ptr = dynamic_cast<DelimiterStringAST*>(delimiters.get());
    if (delimiters_ptr != nullptr) {
        m_lexer.add_delimiters(delimiters_ptr->m_delimiters);
    }
}

void LogParser::add_rules(SchemaAST const* schema_ast) {
    for (auto const& delimiters : schema_ast->m_delimiters) {
        add_delimiters(delimiters);
    }
    vector<uint32_t> delimiters;
    for (uint32_t i = 0; i < cSizeOfByte; i++) {
        if (m_lexer.is_delimiter(i)) {
            delimiters.push_back(i);
        }
    }
    // Currently, required to have delimiters
    if (delimiters.empty()) {
        throw runtime_error("When using --schema-path, \"delimiters:\" line must be used.");
    }
    add_token("newLine", '\n');
    for (unique_ptr<ParserAST> const& parser_ast : schema_ast->m_schema_vars) {
        auto* rule = dynamic_cast<SchemaVarAST*>(parser_ast.get());
        if (rule->m_name == "timestamp") {
            unique_ptr<RegexAST<RegexNFAByteState>> first_timestamp_regex_ast(
                    rule->m_regex_ptr->clone()
            );
            unique_ptr<RegexASTLiteral<RegexNFAByteState>> r1
                    = make_unique<RegexASTLiteral<RegexNFAByteState>>(utf8::cCharStartOfFile);
            add_rule(
                    "firstTimestamp",
                    make_unique<RegexASTCat<RegexNFAByteState>>(
                            std::move(r1),
                            std::move(first_timestamp_regex_ast)
                    )
            );
            unique_ptr<RegexAST<RegexNFAByteState>> newline_timestamp_regex_ast(
                    rule->m_regex_ptr->clone()
            );
            unique_ptr<RegexASTLiteral<RegexNFAByteState>> r2
                    = make_unique<RegexASTLiteral<RegexNFAByteState>>('\n');
            add_rule(
                    "newLineTimestamp",
                    make_unique<RegexASTCat<RegexNFAByteState>>(
                            std::move(r2),
                            std::move(newline_timestamp_regex_ast)
                    )
            );
            // prevent timestamps from going into the dictionary
            continue;
        }
        // transform '.' from any-character into any non-delimiter character
        rule->m_regex_ptr->remove_delimiters_from_wildcard(delimiters);
        // currently, error out if non-timestamp pattern contains a delimiter
        // check if regex contains a delimiter
        bool is_possible_input[cUnicodeMax] = {false};
        rule->m_regex_ptr->set_possible_inputs_to_true(is_possible_input);
        bool contains_delimiter = false;
        uint32_t delimiter_name = 0;
        for (uint32_t delimiter : delimiters) {
            if (is_possible_input[delimiter]) {
                contains_delimiter = true;
                delimiter_name = delimiter;
                break;
            }
        }
        if (contains_delimiter) {
            FileReader schema_reader;
            ErrorCode error_code = schema_reader.try_open(schema_ast->m_file_path);
            if (ErrorCode::Success != error_code) {
                throw std::runtime_error(
                        schema_ast->m_file_path + ":" + to_string(rule->m_line_num + 1)
                        + ": error: '" + rule->m_name
                        + "' has regex pattern which contains delimiter '" + char(delimiter_name)
                        + "'.\n"
                );
            }
            // more detailed debugging based on looking at the file
            string line;
            for (uint32_t i = 0; i <= rule->m_line_num; i++) {
                schema_reader.try_read_to_delimiter('\n', false, false, line);
            }
            int colon_pos = 0;
            for (char i : line) {
                colon_pos++;
                if (i == ':') {
                    break;
                }
            }
            string indent(10, ' ');
            string spaces(colon_pos, ' ');
            string arrows(line.size() - colon_pos, '^');

            throw std::runtime_error(
                    schema_ast->m_file_path + ":" + to_string(rule->m_line_num + 1) + ": error: '"
                    + rule->m_name + "' has regex pattern which contains delimiter '"
                    + char(delimiter_name) + "'.\n" + indent + line + "\n" + indent + spaces
                    + arrows + "\n"
            );
        }
        unique_ptr<RegexASTGroup<RegexNFAByteState>> delimiter_group
                = make_unique<RegexASTGroup<RegexNFAByteState>>(
                        RegexASTGroup<RegexNFAByteState>(delimiters)
                );
        rule->m_regex_ptr = make_unique<RegexASTCat<RegexNFAByteState>>(
                std::move(delimiter_group),
                std::move(rule->m_regex_ptr)
        );
        add_rule(rule->m_name, std::move(rule->m_regex_ptr));
    }
}

auto LogParser::reset() -> void {
    m_input_buffer.reset();
    m_lexer.reset();
    m_lexer.prepend_start_of_file_char(m_input_buffer);
}

auto LogParser::parse_and_generate_metadata(LogParser::ParsingAction& parsing_action) -> ErrorCode {
    ErrorCode error_code = parse(parsing_action);
    if (ErrorCode::Success == error_code) {
        generate_log_event_view_metadata();
    }
    return error_code;
}

auto LogParser::parse(LogParser::ParsingAction& parsing_action) -> ErrorCode {
    std::unique_ptr<LogParserOutputBuffer>& output_buffer = m_log_event_view->m_log_output_buffer;
    if (0 == output_buffer->pos()) {
        output_buffer->set_has_delimiters(m_lexer.get_has_delimiters());
        Token next_token;
        if (m_has_start_of_log) {
            next_token = m_start_of_log_message;
        } else {
            if (ErrorCode err = get_next_symbol(next_token); ErrorCode::Success != err) {
                return err;
            }
            if (false == output_buffer->has_timestamp()
                && next_token.m_type_ids_ptr->at(0) == (int)SymbolID::TokenNewlineTimestampId)
            {
                // TODO: combine the below with found_start_of_next_message
                // into 1 function
                // Increment by 1 because the '\n' character is not part of the
                // next log message
                m_start_of_log_message = next_token;
                if (m_start_of_log_message.m_start_pos == m_start_of_log_message.m_buffer_size - 1)
                {
                    m_start_of_log_message.m_start_pos = 0;
                } else {
                    m_start_of_log_message.m_start_pos++;
                }
                // make a message with just the '\n' character
                next_token.m_end_pos = next_token.m_start_pos + 1;
                next_token.m_type_ids_ptr
                        = &Lexer<RegexNFAByteState, RegexDFAByteState>::cTokenUncaughtStringTypes;
                output_buffer->set_token(1, next_token);
                output_buffer->set_pos(2);
                m_input_buffer.set_consumed_pos(next_token.m_start_pos);
                m_has_start_of_log = true;
                parsing_action = ParsingAction::Compress;
                return ErrorCode::Success;
            }
        }
        if (next_token.m_type_ids_ptr->at(0) == (int)SymbolID::TokenEndID) {
            output_buffer->set_token(0, next_token);
            output_buffer->set_pos(1);
            parsing_action = ParsingAction::CompressAndFinish;
            return ErrorCode::Success;
        }
        if (next_token.m_type_ids_ptr->at(0) == (int)SymbolID::TokenFirstTimestampId
            || next_token.m_type_ids_ptr->at(0) == (int)SymbolID::TokenNewlineTimestampId)
        {
            output_buffer->set_has_timestamp(true);
            output_buffer->set_token(0, next_token);
            output_buffer->set_pos(1);
        } else {
            output_buffer->set_has_timestamp(false);
            output_buffer->set_token(1, next_token);
            output_buffer->set_pos(2);
        }
        m_has_start_of_log = false;
    }

    while (true) {
        Token next_token;
        if (ErrorCode err = get_next_symbol(next_token); ErrorCode::Success != err) {
            return err;
        }
        output_buffer->set_curr_token(next_token);
        int token_type = next_token.m_type_ids_ptr->at(0);
        bool found_start_of_next_message = (output_buffer->has_timestamp()
                                            && token_type == (int)SymbolID::TokenNewlineTimestampId)
                                           || (!output_buffer->has_timestamp()
                                               && next_token.get_char(0) == '\n'
                                               && token_type != (int)SymbolID::TokenNewlineId);
        if (token_type == (int)SymbolID::TokenEndID) {
            parsing_action = ParsingAction::CompressAndFinish;
            return ErrorCode::Success;
        }
        if (false == output_buffer->has_timestamp() && token_type == (int)SymbolID::TokenNewlineId)
        {
            m_input_buffer.set_consumed_pos(output_buffer->get_curr_token().m_end_pos);
            output_buffer->advance_to_next_token();
            parsing_action = ParsingAction::Compress;
            return ErrorCode::Success;
        }
        if (found_start_of_next_message) {
            // increment by 1 because the '\n' character is not part of the next
            // log message
            m_start_of_log_message = output_buffer->get_curr_token();
            if (m_start_of_log_message.m_start_pos == m_start_of_log_message.m_buffer_size - 1) {
                m_start_of_log_message.m_start_pos = 0;
            } else {
                m_start_of_log_message.m_start_pos++;
            }
            // make the last token of the current message the '\n' character
            Token curr_token = output_buffer->get_curr_token();
            curr_token.m_end_pos = curr_token.m_start_pos + 1;
            curr_token.m_type_ids_ptr
                    = &Lexer<RegexNFAByteState, RegexDFAByteState>::cTokenUncaughtStringTypes;
            output_buffer->set_curr_token(curr_token);
            if (0 == m_start_of_log_message.m_start_pos) {
                m_input_buffer.set_consumed_pos(m_input_buffer.storage().size() - 1);
            } else {
                m_input_buffer.set_consumed_pos(m_start_of_log_message.m_start_pos - 1);
            }
            m_has_start_of_log = true;
            output_buffer->advance_to_next_token();
            parsing_action = ParsingAction::Compress;
            return ErrorCode::Success;
        }
        output_buffer->advance_to_next_token();
    }
}

auto LogParser::get_symbol_id(std::string const& symbol) const -> std::optional<uint32_t> {
    if (auto const& it{m_lexer.m_symbol_id.find(symbol)}; it != m_lexer.m_symbol_id.end()) {
        return it->second;
    }
    return std::nullopt;
}

auto LogParser::get_next_symbol(Token& token) -> ErrorCode {
    return m_lexer.scan(m_input_buffer, token);
}

auto LogParser::generate_log_event_view_metadata() -> void {
    uint32_t start = 0;
    if (false == m_log_event_view->m_log_output_buffer->has_timestamp()) {
        start = 1;
    }
    uint32_t first_newline_pos{0};
    for (uint32_t i = start; i < m_log_event_view->m_log_output_buffer->pos(); i++) {
        Token* token = &m_log_event_view->m_log_output_buffer->get_mutable_token(i);
        m_log_event_view->add_token(token->m_type_ids_ptr->at(0), token);
        if (token->get_delimiter() == "\n" && first_newline_pos == 0) {
            first_newline_pos = i;
        }
    }
    // To be a multiline log there must be at least one token between the
    // newline token and the last token in the output buffer.
    if (m_log_event_view->m_log_output_buffer->has_timestamp() && 0 < first_newline_pos
        && first_newline_pos + 1 < m_log_event_view->m_log_output_buffer->pos())
    {
        m_log_event_view->set_multiline(true);
    }
}
}  // namespace log_surgeon
