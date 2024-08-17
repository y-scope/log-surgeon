#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP

#include <algorithm>
#include <array>
#include <cstdint>
#include <gsl/pointers>
#include <gsl/util>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexNFA.hpp>

namespace log_surgeon::finite_automata {

template <typename NFAStateType>
class RegexAST {
public:
    virtual ~RegexAST() = default;
    RegexAST() = default;
    RegexAST(RegexAST const& rhs) = default;
    auto operator=(RegexAST const& rhs) -> RegexAST& = default;
    RegexAST(RegexAST&& rhs) noexcept = default;
    auto operator=(RegexAST&& rhs) noexcept -> RegexAST& = default;

    /**
     * Used for cloning a unique_pointer of base type RegexAST
     * @return RegexAST*
     */
    [[nodiscard]] virtual auto clone() const -> gsl::owner<RegexAST*> = 0;

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule
     * @param is_possible_input
     */
    virtual auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax> is_possible_input
    ) const -> void = 0;

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
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTLiteral*> override {
        return new RegexASTLiteral(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTLiteral at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax> is_possible_input
    ) const -> void override {
        is_possible_input[m_character] = true;
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule, which does
     * nothing as RegexASTLiteral is a leaf node that is not a RegexASTGroup
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard([[maybe_unused]] std::vector<uint32_t>& delimiters
    ) -> void override {
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
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTInteger*> override {
        return new RegexASTInteger(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTInteger at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax> is_possible_input
    ) const -> void override {
        for (uint32_t const i : m_digits) {
            gsl::at(is_possible_input, '0' + i) = true;
        }
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule, which does
     * nothing as RegexASTInteger is a leaf node that is not a RegexASTGroup
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard([[maybe_unused]] std::vector<uint32_t>& delimiters
    ) -> void override {
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
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTGroup*> override {
        return new RegexASTGroup(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTGroup at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax> is_possible_input
    ) const -> void override {
        if (!m_negate) {
            for (auto const& [fst, snd] : m_ranges) {
                for (uint32_t i = fst; i <= snd; i++) {
                    gsl::at(is_possible_input, i) = true;
                }
            }
        } else {
            std::vector<char> inputs(cUnicodeMax, 1);
            for (auto const& [fst, snd] : m_ranges) {
                for (uint32_t i = fst; i <= snd; i++) {
                    inputs[i] = 0;
                }
            }
            for (uint32_t i = 0; i < inputs.size(); i++) {
                if (inputs[i] != 0) {
                    gsl::at(is_possible_input, i) = true;
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
        std::ranges::sort(delimiters);
        if (delimiters[0] != 0) {
            Range const range(0, delimiters[0] - 1);
            m_ranges.push_back(range);
        }
        for (uint32_t i = 1; i < delimiters.size(); i++) {
            if (delimiters[i] - delimiters[i - 1] > 1) {
                Range const range(delimiters[i - 1] + 1, delimiters[i] - 1);
                m_ranges.push_back(range);
            }
        }
        if (delimiters.back() != cUnicodeMax) {
            Range const range(delimiters.back() + 1, cUnicodeMax);
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

    [[nodiscard]] auto is_wildcard() const -> bool { return m_is_wildcard; }

    [[nodiscard]] auto get_negate() const -> bool { return m_negate; }

    [[nodiscard]] auto get_ranges() const -> std::vector<Range> { return m_ranges; }

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
    ~RegexASTOr() override = default;

    RegexASTOr(
            std::unique_ptr<RegexAST<NFAStateType>> left,
            std::unique_ptr<RegexAST<NFAStateType>> right
    );

    RegexASTOr(RegexASTOr const& rhs)
            : m_left(std::unique_ptr<RegexAST<NFAStateType>>(rhs.m_left->clone())),
              m_right(std::unique_ptr<RegexAST<NFAStateType>>(rhs.m_right->clone())) {}

    auto operator=(RegexASTOr const& rhs) -> RegexASTOr& = default;
    RegexASTOr(RegexASTOr&& rhs) noexcept = default;
    auto operator=(RegexASTOr&& rhs) noexcept -> RegexASTOr& = default;

    /**
     * Used for cloning a unique_pointer of type RegexASTOr
     * @return RegexASTOr*
     */
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTOr*> override {
        return new RegexASTOr(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTOr at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax> is_possible_input
    ) const -> void override {
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
    ~RegexASTCat() override = default;

    RegexASTCat(
            std::unique_ptr<RegexAST<NFAStateType>> left,
            std::unique_ptr<RegexAST<NFAStateType>> right
    );

    RegexASTCat(RegexASTCat const& rhs)
            : m_left(std::unique_ptr<RegexAST<NFAStateType>>(rhs.m_left->clone())),
              m_right(std::unique_ptr<RegexAST<NFAStateType>>(rhs.m_right->clone())) {}

    auto operator=(RegexASTCat const& rhs) -> RegexASTCat& = default;
    RegexASTCat(RegexASTCat&& rhs) noexcept = default;
    auto operator=(RegexASTCat&& rhs) noexcept -> RegexASTCat& = default;

    /**
     * Used for cloning a unique_pointer of type RegexASTCat
     * @return RegexASTCat*
     */
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTCat*> override {
        return new RegexASTCat(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTCat at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax> is_possible_input
    ) const -> void override {
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

    [[nodiscard]] auto get_left() const -> std::unique_ptr<RegexAST<NFAStateType>> const& {
        return m_left;
    }

    [[nodiscard]] auto get_right() const -> std::unique_ptr<RegexAST<NFAStateType>> const& {
        return m_right;
    }

private:
    std::unique_ptr<RegexAST<NFAStateType>> m_left;
    std::unique_ptr<RegexAST<NFAStateType>> m_right;
};

template <typename NFAStateType>
class RegexASTMultiplication : public RegexAST<NFAStateType> {
public:
    ~RegexASTMultiplication() override = default;

    RegexASTMultiplication(
            std::unique_ptr<RegexAST<NFAStateType>> operand,
            uint32_t min,
            uint32_t max
    );

    RegexASTMultiplication(RegexASTMultiplication const& rhs)
            : m_operand(std::unique_ptr<RegexAST<NFAStateType>>(rhs.m_operand->clone())),
              m_min(rhs.m_min),
              m_max(rhs.m_max) {}

    auto operator=(RegexASTMultiplication const& rhs) -> RegexASTMultiplication& = default;
    RegexASTMultiplication(RegexASTMultiplication&& rhs) noexcept = default;
    auto operator=(RegexASTMultiplication&& rhs) noexcept -> RegexASTMultiplication& = default;

    /**
     * Used for cloning a unique_pointer of type RegexASTMultiplication
     * @return RegexASTMultiplication*
     */
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTMultiplication*> override {
        return new RegexASTMultiplication(*this);
    }

    /**
     * Sets is_possible_input to specify which utf8 characters are allowed in a
     * lexer rule containing RegexASTMultiplication at a leaf node in its AST
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax> is_possible_input
    ) const -> void override {
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

    [[nodiscard]] auto get_operand() const -> std::unique_ptr<RegexAST<NFAStateType>> const& {
        return m_operand;
    }

    [[nodiscard]] auto get_min() const -> uint32_t { return m_min; }

    [[nodiscard]] auto get_max() const -> uint32_t { return m_max; }

private:
    std::unique_ptr<RegexAST<NFAStateType>> m_operand;
    uint32_t m_min;
    uint32_t m_max;
};

template <typename NFAStateType>
class RegexASTCapture : public RegexAST<NFAStateType> {
public:
    ~RegexASTCapture() override = default;

    RegexASTCapture(std::string group_name, std::unique_ptr<RegexAST<NFAStateType>> group_regex_ast)
            : m_group_name(std::move(group_name)),
              m_group_regex_ast(std::move(group_regex_ast)) {}

    RegexASTCapture(RegexASTCapture const& rhs)
            : m_group_name(rhs.m_group_name),
              m_group_regex_ast(
                      std::unique_ptr<RegexAST<NFAStateType>>(rhs.m_group_regex_ast->clone())
              ) {}

    auto operator=(RegexASTCapture const& rhs) -> RegexASTCapture& = default;
    RegexASTCapture(RegexASTCapture&& rhs) noexcept = default;
    auto operator=(RegexASTCapture&& rhs) noexcept -> RegexASTCapture& = default;

    /**
     * Used for cloning a `unique_pointer` of type `RegexASTCapture`.
     * @return RegexASTCapture*
     */
    [[nodiscard]] auto clone() const -> gsl::owner<RegexASTCapture*> override {
        return new RegexASTCapture(*this);
    }

    /**
     * Sets `is_possible_input` to specify which utf8 characters are allowed in a
     * lexer rule containing `RegexASTCapture` at a leaf node in its AST.
     * @param is_possible_input
     */
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax> is_possible_input
    ) const -> void override {
        m_group_regex_ast->set_possible_inputs_to_true(is_possible_input);
    }

    /**
     * Transforms '.' to to be any non-delimiter in a lexer rule if
     * `RegexASTGroup` with `.` is a descendant of this `RegexASTCapture` node.
     * @param delimiters
     */
    auto remove_delimiters_from_wildcard(std::vector<uint32_t>& delimiters) -> void override {
        m_group_regex_ast->remove_delimiters_from_wildcard(delimiters);
    }

    /**
     * Adds the needed `RegexNFA::states` to the passed in nfa to handle a
     * `RegexASTCapture` before transitioning to a pre-tagged `end_state`.
     * @param nfa
     * @param end_state
     */
    auto add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) -> void override;

    [[nodiscard]] auto get_group_name() const -> std::string const& { return m_group_name; }

    [[nodiscard]] auto get_group_regex_ast(
    ) const -> std::unique_ptr<RegexAST<NFAStateType>> const& {
        return m_group_regex_ast;
    }

private:
    std::string m_group_name;
    std::unique_ptr<RegexAST<NFAStateType>> m_group_regex_ast;
};

}  // namespace log_surgeon::finite_automata

#include "RegexAST.tpp"

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP
