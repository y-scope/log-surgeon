#ifndef LOG_SURGEON_LEXER_HPP
#define LOG_SURGEON_LEXER_HPP

#include <array>
#include <cstdint>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/Dfa.hpp>
#include <log_surgeon/finite_automata/DfaState.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>
#include <log_surgeon/finite_automata/RegexAST.hpp>
#include <log_surgeon/LexicalRule.hpp>
#include <log_surgeon/ParserInputBuffer.hpp>
#include <log_surgeon/Token.hpp>
#include <log_surgeon/types.hpp>

namespace log_surgeon {
template <typename TypedNfaState, typename TypedDfaState>
class Lexer {
public:
    static inline std::vector<uint32_t> const cTokenEndTypes = {(uint32_t)SymbolId::TokenEnd};
    static inline std::vector<uint32_t> const cTokenUncaughtStringTypes
            = {(uint32_t)SymbolId::TokenUncaughtString};

    /**
     * Add a delimiters line from the schema to the lexer
     * @param delimiters
     */
    auto add_delimiters(std::vector<uint32_t> const& delimiters) -> void;

    /**
     * Add lexical rule to the lexer's list of rules
     * @param rule_id
     * @param rule
     */
    auto add_rule(rule_id_t rule_id, std::unique_ptr<finite_automata::RegexAST<TypedNfaState>> rule)
            -> void;

    /**
     * Return regex pattern for a rule name
     * @param variable_id
     * @return finite_automata::RegexAST*
     */
    auto get_rule(uint32_t variable_id) -> finite_automata::RegexAST<TypedNfaState>*;

    /**
     * Generate DFA for lexer.
     * @throw std::invalid_argument if `m_rules` contains multipe captures with the same name.
     */
    auto generate() -> void;

    /**
     * Reset the lexer to start a new lexing (reset buffers, reset vars tracking
     * positions)
     */
    auto reset() -> void;

    /**
     * Set the lexer state as if it had already read a delimiter (used for
     * treating start of file as a delimiter)
     * @param input_buffer containing the data to be lexed
     */
    auto prepend_start_of_file_char(ParserInputBuffer& input_buffer) -> void;

    /**
     * Flip lexer states to match static buffer flipping.
     * @param old_storage_size The previous buffer size used to calculate the
     * new states inside the flipped buffer.
     */
    auto flip_states(uint32_t old_storage_size) -> void;

    /**
     * Gets next token from the input string
     * If next token is an uncaught string, the next variable token is already
     * prepped to be returned on the next call
     * @param input_buffer
     * @param Token&
     * @return ErrorCode::Success
     * @return ErrorCode::BufferOutOfBounds if end of input reached before
     * lexing a token.
     * @throw runtime_error("Input buffer about to overflow")
     */
    auto scan(ParserInputBuffer& input_buffer, Token& token) -> ErrorCode;

    /**
     * scan(), but with wild wildcards in the input string (for search)
     * @param input_buffer
     * @param wildcard
     * @return Token
     * @throw runtime_error("Input buffer about to overflow")
     */
    auto
    scan_with_wildcard(ParserInputBuffer& input_buffer, char wildcard, Token& token) -> ErrorCode;

    /**
     * Grows the capacity of the passed in input buffer if it is not large
     * enough to store the contents of an entire LogEvent. Then, adjusts any
     * values being tracked in the lexer related to the input buffer if needed.
     * @param parser_input_buffer Buffer which size needs to be checked.
     */
    auto increase_buffer_capacity(ParserInputBuffer& input_buffer) -> void;

    [[nodiscard]] auto get_has_delimiters() const -> bool const& { return m_has_delimiters; }

    [[nodiscard]] auto is_delimiter(uint8_t byte) const -> bool const& {
        return m_is_delimiter[byte];
    }

    // First character of any variable in the schema
    [[nodiscard]] auto is_first_char(uint8_t byte) const -> bool const& {
        return m_is_first_char[byte];
    }

    [[nodiscard]] auto get_dfa(
    ) const -> std::unique_ptr<finite_automata::Dfa<TypedDfaState, TypedNfaState>> const& {
        return m_dfa;
    }

    /**
     * @param rule_id ID associated with a rule.
     * @return A vector of capture IDs corresponding to each rule that contain the variable on
     * success.
     * @return std::nullopt if the variable is never captured in any rule.
     */
    [[nodiscard]] auto get_capture_ids_from_rule_id(rule_id_t const rule_id
    ) const -> std::optional<std::vector<capture_id_t>> {
        if (m_rule_id_to_capture_ids.contains(rule_id)) {
            return m_rule_id_to_capture_ids.at(rule_id);
        }
        return std::nullopt;
    }

    /**
     * @param capture_id ID associated with a capture within a rule.
     * @return The start and end tag of the capture on success.
     * @return std::nullopt if no capture is associated with the given capture ID.
     */
    [[nodiscard]] auto get_tag_id_pair_from_capture_id(capture_id_t const capture_id
    ) const -> std::optional<std::pair<tag_id_t, tag_id_t>> {
        if (m_capture_id_to_tag_id_pair.contains(capture_id)) {
            return m_capture_id_to_tag_id_pair.at(capture_id);
        }
        return std::nullopt;
    }

    /**
     * @param tag_id ID associated with a tag.
     * @return The final register ID tracking the value of the tag ID during DFA simulation on
     * success.
     * @return std::nullopt if no tag is associated with the given tag ID.
     */
    [[nodiscard]] auto get_reg_id_from_tag_id(tag_id_t const tag_id
    ) const -> std::optional<reg_id_t> {
        if (m_tag_to_final_reg_id.contains(tag_id)) {
            return m_tag_to_final_reg_id.at(tag_id);
        }
        return std::nullopt;
    }

    /**
     * @param capture_id ID associated with a capture within a rule.
     * @return The start and end final register IDs tracking the position of the capture on success.
     * @return std::nullopt if no capture is associated with the given capture ID.
     */
    [[nodiscard]] auto get_reg_ids_from_capture_id(capture_id_t const capture_id
    ) const -> std::optional<std::pair<reg_id_t, reg_id_t>> {
        auto const optional_tag_id_pair{get_tag_id_pair_from_capture_id(capture_id)};
        if (false == optional_tag_id_pair.has_value()) {
            return std::nullopt;
        }
        auto const [start_tag_id, end_tag_id]{optional_tag_id_pair.value()};

        auto const optional_start_reg_id{get_reg_id_from_tag_id(start_tag_id)};
        if (false == optional_start_reg_id.has_value()) {
            return std::nullopt;
        }

        auto const optional_end_reg_id{get_reg_id_from_tag_id(end_tag_id)};
        if (false == optional_end_reg_id.has_value()) {
            return std::nullopt;
        }

        return {optional_start_reg_id.value(), optional_end_reg_id.value()};
    }

    std::unordered_map<std::string, rule_id_t> m_symbol_id;
    std::unordered_map<rule_id_t, std::string> m_id_symbol;

private:
    /**
     * Get next character from the input buffer
     * @return unsigned char
     */
    auto get_next_character() -> unsigned char;

    uint32_t m_match_pos{0};
    uint32_t m_start_pos{0};
    uint32_t m_match_line{0};
    uint32_t m_last_match_pos{0};
    uint32_t m_last_match_line{0};
    bool m_match{false};
    std::vector<uint32_t> const* m_type_ids{nullptr};
    std::set<uint32_t> m_type_ids_set;
    std::array<bool, cSizeOfByte> m_is_delimiter{false};
    std::array<bool, cSizeOfByte> m_is_first_char{false};
    std::vector<LexicalRule<TypedNfaState>> m_rules;
    uint32_t m_line{0};
    bool m_has_delimiters{false};
    std::unique_ptr<finite_automata::Dfa<TypedDfaState, TypedNfaState>> m_dfa;
    bool m_asked_for_more_data{false};
    TypedDfaState const* m_prev_state{nullptr};
    std::unordered_map<rule_id_t, std::vector<capture_id_t>> m_rule_id_to_capture_ids;
    std::unordered_map<capture_id_t, std::pair<tag_id_t, tag_id_t>> m_capture_id_to_tag_id_pair;
    std::map<tag_id_t, reg_id_t> m_tag_to_final_reg_id;
};

namespace lexers {
using ByteLexer = Lexer<finite_automata::ByteNfaState, finite_automata::ByteDfaState>;
using Utf8Lexer = Lexer<finite_automata::Utf8NfaState, finite_automata::Utf8DfaState>;
}  // namespace lexers
}  // namespace log_surgeon

#include "Lexer.tpp"

#endif  // LOG_SURGEON_LEXER_HPP
