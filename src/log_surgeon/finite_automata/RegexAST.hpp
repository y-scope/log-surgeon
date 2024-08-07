#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP

#include <algorithm>
#include <cstdint>
#include <memory>
#include <set>
#include <utility>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexNFA.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

namespace log_surgeon::finite_automata {

template <typename NFAStateType>
class RegexAST {
public:
    virtual ~RegexAST() = default;

    /**
     * Used for cloning a unique_pointer of base type RegexAST
     * @return RegexAST*
     */
    [[nodiscard]] virtual auto clone() const -> RegexAST* = 0;

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule
     * @param is_possible_input
     */
    virtual auto set_possible_inputs_to_true(bool is_possible_input[]) const -> void = 0;

    /**
     * transform '.' from any-character into any non-delimiter in a lexer rule
     * @param delimiters
     */
    virtual auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void = 0;

    /**
     * Add the needed RegexNFA::states to the passed in nfa to handle the
     * current node before transitioning to a pre-tagged end_state
     * @param nfa
     * @param end_state
     */
    virtual auto add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) -> void = 0;
};

template <typename NFAStateType>
class RegexASTLiteral : public RegexAST<NFAStateType> {
public:
    explicit RegexASTLiteral(uint32_t character);

    /**
     * Used for cloning a unique_pointer of type RegexASTLiteral
     * @return RegexASTLiteral*
     */
    [[nodiscard]] auto clone() const -> RegexASTLiteral* override {
        return new RegexASTLiteral(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTLiteral at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(bool is_possible_input[]) const -> void override {
        is_possible_input[m_character] = true;
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule, which does
     * nothing as RegexASTLiteral is a leaf node that is not a RegexASTGroup
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& /* delimiters */) -> void override {
        // Do nothing
    }

    /**
     * Add the needed RegexNFA::states to the passed in nfa to handle a
     * RegexASTLiteral before transitioning to a pre-tagged end_state
     * @param nfa
     * @param end_state
     */
    auto add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) -> void override;

    [[nodiscard]] auto get_character() const -> uint32_t const& { return m_character; }

private:
    uint32_t m_character;
};

template <typename NFAStateType>
class RegexASTInteger : public RegexAST<NFAStateType> {
public:
    explicit RegexASTInteger(uint32_t digit);

    RegexASTInteger(RegexASTInteger* left, uint32_t digit);

    /**
     * Used for cloning a unique_pointer of type RegexASTInteger
     * @return RegexASTInteger*
     */
    [[nodiscard]] auto clone() const -> RegexASTInteger* override {
        return new RegexASTInteger(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTInteger at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(bool is_possible_input[]) const -> void override {
        for (uint32_t i : m_digits) {
            is_possible_input[i + '0'] = true;
        }
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule, which does
     * nothing as RegexASTInteger is a leaf node that is not a RegexASTGroup
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& /* delimiters */) -> void override {
        // Do nothing
    }

    /**
     * Add the needed RegexNFA::states to the passed in nfa to handle a
     * RegexASTInteger before transitioning to a pre-tagged end_state
     * @param nfa
     * @param end_state
     */
    auto add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) -> void override;

    [[nodiscard]] auto get_digits() const -> std::vector<uint32_t> const& { return m_digits; }

    [[nodiscard]] auto get_digit(uint32_t i) const -> uint32_t const& { return m_digits[i]; }

private:
    std::vector<uint32_t> m_digits;
};

template <typename NFAStateType>
class RegexASTGroup : public RegexAST<NFAStateType> {
public:
    using Range = std::pair<uint32_t, uint32_t>;

    RegexASTGroup();

    RegexASTGroup(RegexASTGroup* left, RegexASTLiteral<NFAStateType>* right);

    RegexASTGroup(RegexASTGroup* left, RegexASTGroup* right);

    explicit RegexASTGroup(RegexASTLiteral<NFAStateType>* right);

    explicit RegexASTGroup(RegexASTGroup* right);

    RegexASTGroup(RegexASTLiteral<NFAStateType>* left, RegexASTLiteral<NFAStateType>* right);

    RegexASTGroup(uint32_t min, uint32_t max);

    explicit RegexASTGroup(std::vector<uint32_t> const& literals);

    /**
     * Used for cloning a unique_pointer of type RegexASTGroup
     * @return RegexASTGroup*
     */
    [[nodiscard]] auto clone() const -> RegexASTGroup* override { return new RegexASTGroup(*this); }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTGroup at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(bool is_possible_input[]) const -> void override {
        if (!m_negate) {
            for (Range range : m_ranges) {
                for (uint32_t i = range.first; i <= range.second; i++) {
                    is_possible_input[i] = true;
                }
            }
        } else {
            std::vector<char> inputs(cUnicodeMax, 1);
            for (Range range : m_ranges) {
                for (uint32_t i = range.first; i <= range.second; i++) {
                    inputs[i] = 0;
                }
            }
            for (uint32_t i = 0; i < inputs.size(); i++) {
                if (inputs[i] != 0) {
                    is_possible_input[i] = true;
                }
            }
        }
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule if this
     * RegexASTGroup node contains `.` (is a wildcard group)
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void override {
        if (!m_is_wildcard) {
            return;
        }
        if (delimiters.empty()) {
            return;
        }
        m_ranges.clear();
        std::sort(delimiters.begin(), delimiters.end());
        if (delimiters[0] != 0) {
            Range range(0, delimiters[0] - 1);
            m_ranges.push_back(range);
        }
        for (uint32_t i = 1; i < delimiters.size(); i++) {
            if (delimiters[i] - delimiters[i - 1] > 1) {
                Range range(delimiters[i - 1] + 1, delimiters[i] - 1);
                m_ranges.push_back(range);
            }
        }
        if (delimiters.back() != cUnicodeMax) {
            Range range(delimiters.back() + 1, cUnicodeMax);
            m_ranges.push_back(range);
        }
    }

    /**
     * Add the needed RegexNFA::states to the passed in nfa to handle a
     * RegexASTGroup before transitioning to a pre-tagged end_state
     * @param nfa
     * @param end_state
     */
    auto add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) -> void override;

    auto add_range(uint32_t min, uint32_t max) -> void { m_ranges.emplace_back(min, max); }

    auto add_literal(uint32_t literal) -> void { m_ranges.emplace_back(literal, literal); }

    auto set_is_wildcard_true() -> void { m_is_wildcard = true; }

private:
    /**
     * Merges multiple ranges such that the resulting m_ranges is sorted and
     * non-overlapping @param ranges
     * @return std::vector<Range>
     */
    static auto merge(std::vector<Range> const& ranges) -> std::vector<Range>;

    /**
     * Takes the compliment (in the cast of regex `^` at the start of a group)
     * of multiple ranges such that m_ranges is sorted and non-overlapping
     * @param ranges
     * @return std::vector<Range>
     */
    static auto complement(std::vector<Range> const& ranges) -> std::vector<Range>;

    bool m_is_wildcard{false};
    bool m_negate{true};
    std::vector<Range> m_ranges;
};

template <typename NFAStateType>
class RegexASTOr : public RegexAST<NFAStateType> {
public:
    RegexASTOr(
            std::unique_ptr<RegexAST<NFAStateType>> /*left*/,
            std::unique_ptr<RegexAST<NFAStateType>> /*right*/
    );

    RegexASTOr(RegexASTOr const& rhs)
            : m_left(std::unique_ptr<RegexAST<NFAStateType>>(rhs.m_left->clone())),
              m_right(std::unique_ptr<RegexAST<NFAStateType>>(rhs.m_right->clone())) {}

    /**
     * Used for cloning a unique_pointer of type RegexASTOr
     * @return RegexASTOr*
     */
    [[nodiscard]] auto clone() const -> RegexASTOr* override { return new RegexASTOr(*this); }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTOr at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(bool is_possible_input[]) const -> void override {
        m_left->set_possible_inputs_to_true(is_possible_input);
        m_right->set_possible_inputs_to_true(is_possible_input);
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule if
     * RegexASTGroup with `.` is a descendant of this RegexASTOr node
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void override {
        m_left->remove_delimiters_from_wildcard(delimiters);
        m_right->remove_delimiters_from_wildcard(delimiters);
    }

    /**
     * Add the needed RegexNFA::states to the passed in nfa to handle a
     * RegexASTOr before transitioning to a pre-tagged end_state
     * @param nfa
     * @param end_state
     */
    auto add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) -> void override;

private:
    std::unique_ptr<RegexAST<NFAStateType>> m_left;
    std::unique_ptr<RegexAST<NFAStateType>> m_right;
};

template <typename NFAStateType>
class RegexASTCat : public RegexAST<NFAStateType> {
public:
    RegexASTCat(
            std::unique_ptr<RegexAST<NFAStateType>> /*left*/,
            std::unique_ptr<RegexAST<NFAStateType>> /*right*/
    );

    RegexASTCat(RegexASTCat const& rhs)
            : m_left(std::unique_ptr<RegexAST<NFAStateType>>(rhs.m_left->clone())),
              m_right(std::unique_ptr<RegexAST<NFAStateType>>(rhs.m_right->clone())) {}

    /**
     * Used for cloning a unique_pointer of type RegexASTCat
     * @return RegexASTCat*
     */
    [[nodiscard]] auto clone() const -> RegexASTCat* override { return new RegexASTCat(*this); }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTCat at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(bool is_possible_input[]) const -> void override {
        m_left->set_possible_inputs_to_true(is_possible_input);
        m_right->set_possible_inputs_to_true(is_possible_input);
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule if
     * RegexASTGroup with `.` is a descendant of this RegexASTCat node
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void override {
        m_left->remove_delimiters_from_wildcard(delimiters);
        m_right->remove_delimiters_from_wildcard(delimiters);
    }

    /**
     * Add the needed RegexNFA::states to the passed in nfa to handle a
     * RegexASTCat before transitioning to a pre-tagged end_state
     * @param nfa
     * @param end_state
     */
    auto add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) -> void override;

private:
    std::unique_ptr<RegexAST<NFAStateType>> m_left;
    std::unique_ptr<RegexAST<NFAStateType>> m_right;
};

template <typename NFAStateType>
class RegexASTMultiplication : public RegexAST<NFAStateType> {
public:
    RegexASTMultiplication(
            std::unique_ptr<RegexAST<NFAStateType>> operand,
            uint32_t min,
            uint32_t max
    );

    RegexASTMultiplication(RegexASTMultiplication const& rhs)
            : m_operand(std::unique_ptr<RegexAST<NFAStateType>>(rhs.m_operand->clone())),
              m_min(rhs.m_min),
              m_max(rhs.m_max) {}

    /**
     * Used for cloning a unique_pointer of type RegexASTMultiplication
     * @return RegexASTMultiplication*
     */
    [[nodiscard]] auto clone() const -> RegexASTMultiplication* override {
        return new RegexASTMultiplication(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTMultiplication at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(bool is_possible_input[]) const -> void override {
        m_operand->set_possible_inputs_to_true(is_possible_input);
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule if
     * RegexASTGroup with `.` is a descendant of this RegexASTMultiplication
     * node
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void override {
        m_operand->remove_delimiters_from_wildcard(delimiters);
    }

    /**
     * Add the needed RegexNFA::states to the passed in nfa to handle a
     * RegexASTMultiplication before transitioning to a pre-tagged end_state
     * @param nfa
     * @param end_state
     */
    auto add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) -> void override;

    [[nodiscard]] auto is_infinite() const -> bool { return this->m_max == 0; }

private:
    std::unique_ptr<RegexAST<NFAStateType>> m_operand;
    uint32_t m_min;
    uint32_t m_max;
};
}  // namespace log_surgeon::finite_automata

#include "RegexAST.tpp"

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP
