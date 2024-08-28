#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP

#include <algorithm>
#include <array>
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <gsl/pointers>
#include <memory>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexNFA.hpp>

namespace log_surgeon::finite_automata {

template <typename NFAStateType>
class RegexAST {
public:
    RegexAST() = default;
    virtual ~RegexAST() = default;

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
    virtual auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax>& is_possible_input
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

protected:
    RegexAST(RegexAST const& rhs) = default;
    auto operator=(RegexAST const& rhs) -> RegexAST& = default;
    RegexAST(RegexAST&& rhs) noexcept = default;
    auto operator=(RegexAST&& rhs) noexcept -> RegexAST& = default;
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
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax>& is_possible_input
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
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax>& is_possible_input
    ) const -> void override {
        for (uint32_t const i : m_digits) {
            is_possible_input.at('0' + i) = true;
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

    explicit RegexASTGroup(RegexASTLiteral<NFAStateType> const* right);

    explicit RegexASTGroup(RegexASTGroup const* right);

    RegexASTGroup(RegexASTGroup const* left, RegexASTLiteral<NFAStateType> const* right);

    RegexASTGroup(RegexASTGroup const* left, RegexASTGroup const* right);

    RegexASTGroup(
            RegexASTLiteral<NFAStateType> const* left,
            RegexASTLiteral<NFAStateType> const* right
    );

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
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax>& is_possible_input
    ) const -> void override {
        if (!m_negate) {
            for (auto const& [begin, end] : m_ranges) {
                for (uint32_t i = begin; i <= end; i++) {
                    is_possible_input.at(i) = true;
                }
            }
        } else {
            std::vector<char> inputs(cUnicodeMax, 1);
            for (auto const& [begin, end] : m_ranges) {
                for (uint32_t i = begin; i <= end; i++) {
                    inputs[i] = 0;
                }
            }
            for (uint32_t i = 0; i < inputs.size(); i++) {
                if (inputs[i] != 0) {
                    is_possible_input.at(i) = true;
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
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax>& is_possible_input
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
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax>& is_possible_input
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
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax>& is_possible_input
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
    auto set_possible_inputs_to_true(std::array<bool, cUnicodeMax>& is_possible_input
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

template <typename NFAStateType>
RegexASTLiteral<NFAStateType>::RegexASTLiteral(uint32_t character) : m_character(character) {}

template <typename NFAStateType>
void RegexASTLiteral<NFAStateType>::add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) {
    nfa->add_root_interval(Interval(m_character, m_character), end_state);
}

template <typename NFAStateType>
RegexASTInteger<NFAStateType>::RegexASTInteger(uint32_t digit) {
    digit = digit - '0';
    m_digits.push_back(digit);
}

template <typename NFAStateType>
RegexASTInteger<NFAStateType>::RegexASTInteger(RegexASTInteger* left, uint32_t digit)
        : m_digits(std::move(left->m_digits)) {
    digit = digit - '0';
    m_digits.push_back(digit);
}

template <typename NFAStateType>
void RegexASTInteger<NFAStateType>::add(
        [[maybe_unused]] RegexNFA<NFAStateType>* nfa,
        [[maybe_unused]] NFAStateType* end_state
) {
    throw std::runtime_error("Unsupported");
}

template <typename NFAStateType>
RegexASTOr<NFAStateType>::RegexASTOr(
        std::unique_ptr<RegexAST<NFAStateType>> left,
        std::unique_ptr<RegexAST<NFAStateType>> right
)
        : m_left(std::move(left)),
          m_right(std::move(right)) {}

template <typename NFAStateType>
void RegexASTOr<NFAStateType>::add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) {
    m_left->add(nfa, end_state);
    m_right->add(nfa, end_state);
}

template <typename NFAStateType>
RegexASTCat<NFAStateType>::RegexASTCat(
        std::unique_ptr<RegexAST<NFAStateType>> left,
        std::unique_ptr<RegexAST<NFAStateType>> right
)
        : m_left(std::move(left)),
          m_right(std::move(right)) {}

template <typename NFAStateType>
void RegexASTCat<NFAStateType>::add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) {
    NFAStateType* saved_root = nfa->get_root();
    NFAStateType* intermediate_state = nfa->new_state();
    m_left->add(nfa, intermediate_state);
    nfa->set_root(intermediate_state);
    m_right->add(nfa, end_state);
    nfa->set_root(saved_root);
}

template <typename NFAStateType>
RegexASTMultiplication<NFAStateType>::RegexASTMultiplication(
        std::unique_ptr<RegexAST<NFAStateType>> operand,
        uint32_t const min,
        uint32_t const max
)
        : m_operand(std::move(operand)),
          m_min(min),
          m_max(max) {}

template <typename NFAStateType>
void RegexASTMultiplication<NFAStateType>::add(
        RegexNFA<NFAStateType>* nfa,
        NFAStateType* end_state
) {
    NFAStateType* saved_root = nfa->get_root();
    if (this->m_min == 0) {
        nfa->get_root()->add_epsilon_transition(end_state);
    } else {
        for (uint32_t i = 1; i < this->m_min; i++) {
            NFAStateType* intermediate_state = nfa->new_state();
            m_operand->add(nfa, intermediate_state);
            nfa->set_root(intermediate_state);
        }
        m_operand->add(nfa, end_state);
    }
    if (this->is_infinite()) {
        nfa->set_root(end_state);
        m_operand->add(nfa, end_state);
    } else if (this->m_max > this->m_min) {
        if (this->m_min != 0) {
            NFAStateType* intermediate_state = nfa->new_state();
            m_operand->add(nfa, intermediate_state);
            nfa->set_root(intermediate_state);
        }
        for (uint32_t i = this->m_min + 1; i < this->m_max; ++i) {
            m_operand->add(nfa, end_state);
            NFAStateType* intermediate_state = nfa->new_state();
            m_operand->add(nfa, intermediate_state);
            nfa->set_root(intermediate_state);
        }
        m_operand->add(nfa, end_state);
    }
    nfa->set_root(saved_root);
}

template <typename NFAStateType>
void RegexASTCapture<NFAStateType>::add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) {
    m_group_regex_ast->add(nfa, end_state);
}

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup() = default;

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup(
        RegexASTGroup const* left,
        RegexASTLiteral<NFAStateType> const* right
) {
    if (right == nullptr) {
        throw std::runtime_error("RegexASTGroup1: right == nullptr: A bracket expression in the "
                                 "schema contains illegal characters, remember to escape special "
                                 "characters. Refer to README-Schema.md for more details.");
    }
    m_negate = left->m_negate;
    m_ranges = left->m_ranges;
    m_ranges.emplace_back(right->get_character(), right->get_character());
}

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup(RegexASTGroup const* left, RegexASTGroup const* right)
        : m_negate(left->m_negate),
          m_ranges(left->m_ranges) {
    assert(right->m_ranges.size() == 1);  // Only add LiteralRange
    m_ranges.push_back(right->m_ranges[0]);
}

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup(RegexASTLiteral<NFAStateType> const* right) {
    if (right == nullptr) {
        throw std::runtime_error("RegexASTGroup2: right == nullptr: A bracket expression in the "
                                 "schema contains illegal characters, remember to escape special "
                                 "characters. Refer to README-Schema.md for more details.");
    }
    m_negate = false;
    m_ranges.emplace_back(right->get_character(), right->get_character());
}

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup(RegexASTGroup const* right) : m_negate(false) {
    assert(right->m_ranges.size() == 1);  // Only add LiteralRange
    m_ranges.push_back(right->m_ranges[0]);
}

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup(
        RegexASTLiteral<NFAStateType> const* left,
        RegexASTLiteral<NFAStateType> const* right
) {
    if (left == nullptr || right == nullptr) {
        throw std::runtime_error(
                "RegexASTGroup3: left == nullptr || right == nullptr: A bracket expression in the "
                "schema contains illegal characters, remember to escape special characters. Refer "
                "to README-Schema.md for more details."
        );
    }
    m_negate = false;
    assert(right->get_character() > left->get_character());
    m_ranges.emplace_back(left->get_character(), right->get_character());
}

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup(std::vector<uint32_t> const& literals)
        : m_negate(false) {
    for (uint32_t literal : literals) {
        m_ranges.emplace_back(literal, literal);
    }
}

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup(uint32_t min, uint32_t max) : m_negate(false) {
    m_ranges.emplace_back(min, max);
}

// ranges must be sorted
template <typename NFAStateType>
auto RegexASTGroup<NFAStateType>::merge(std::vector<Range> const& ranges) -> std::vector<Range> {
    std::vector<Range> merged_ranges;
    if (ranges.empty()) {
        return merged_ranges;
    }
    Range cur = ranges[0];
    for (size_t i = 1; i < ranges.size(); i++) {
        auto const& range = ranges[i];
        if (range.first <= cur.second + 1) {
            cur.second = std::max(range.second, cur.second);
        } else {
            merged_ranges.push_back(cur);
            cur = range;
        }
    }
    merged_ranges.push_back(cur);
    return merged_ranges;
}

// ranges must be sorted and non-overlapping
template <typename NFAStateType>
auto RegexASTGroup<NFAStateType>::complement(std::vector<Range> const& ranges
) -> std::vector<Range> {
    std::vector<Range> complemented;
    uint32_t low = 0;
    for (auto const& [begin, end] : ranges) {
        if (begin > 0) {
            complemented.emplace_back(low, begin - 1);
        }
        low = end + 1;
    }
    if (low > 0) {
        complemented.emplace_back(low, cUnicodeMax);
    }
    return complemented;
}

template <typename NFAStateType>
void RegexASTGroup<NFAStateType>::add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) {
    std::sort(this->m_ranges.begin(), this->m_ranges.end());
    std::vector<Range> merged_ranges = RegexASTGroup::merge(this->m_ranges);
    if (this->m_negate) {
        merged_ranges = complement(merged_ranges);
    }
    for (auto const& [begin, end] : merged_ranges) {
        nfa->get_root()->add_interval(Interval(begin, end), end_state);
    }
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_HPP
