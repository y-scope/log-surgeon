#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_TPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_TPP

#include <algorithm>
#include <cassert>
#include <cstdint>
#include <stdexcept>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexNFA.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>

namespace log_surgeon::finite_automata {

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
RegexASTInteger<NFAStateType>::RegexASTInteger(RegexASTInteger<NFAStateType>* left, uint32_t digit)
        : m_digits(std::move(left->m_digits)) {
    digit = digit - '0';
    m_digits.push_back(digit);
}

template <typename NFAStateType>
void RegexASTInteger<
        NFAStateType>::add(RegexNFA<NFAStateType>* /* nfa */, NFAStateType* /* end_state */) {
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
        uint32_t min,
        uint32_t max
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
        for (uint32_t i = this->m_min + 1; i < this->m_max; i++) {
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
RegexASTGroup<NFAStateType>::RegexASTGroup() = default;

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup(
        RegexASTGroup<NFAStateType>* left,
        RegexASTLiteral<NFAStateType>* right
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
RegexASTGroup<NFAStateType>::RegexASTGroup(
        RegexASTGroup<NFAStateType>* left,
        RegexASTGroup<NFAStateType>* right
)
        : m_negate(left->m_negate),
          m_ranges(left->m_ranges) {
    assert(right->m_ranges.size() == 1);  // Only add LiteralRange
    m_ranges.push_back(right->m_ranges[0]);
}

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup(RegexASTLiteral<NFAStateType>* right) {
    if (right == nullptr) {
        throw std::runtime_error("RegexASTGroup2: right == nullptr: A bracket expression in the "
                                 "schema contains illegal characters, remember to escape special "
                                 "characters. Refer to README-Schema.md for more details.");
    }
    m_negate = false;
    m_ranges.emplace_back(right->get_character(), right->get_character());
}

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup(RegexASTGroup<NFAStateType>* right) : m_negate(false) {
    assert(right->m_ranges.size() == 1);  // Only add LiteralRange
    m_ranges.push_back(right->m_ranges[0]);
}

template <typename NFAStateType>
RegexASTGroup<NFAStateType>::RegexASTGroup(
        RegexASTLiteral<NFAStateType>* left,
        RegexASTLiteral<NFAStateType>* right
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
auto RegexASTGroup<NFAStateType>::merge(std::vector<Range> const& ranges)
        -> std::vector<typename RegexASTGroup<NFAStateType>::Range> {
    std::vector<Range> merged;
    if (ranges.empty()) {
        return merged;
    }
    Range cur = ranges[0];
    for (size_t i = 1; i < ranges.size(); i++) {
        Range r = ranges[i];
        if (r.first <= cur.second + 1) {
            cur.second = std::max(r.second, cur.second);
        } else {
            merged.push_back(cur);
            cur = r;
        }
    }
    merged.push_back(cur);
    return merged;
}

// ranges must be sorted and non-overlapping
template <typename NFAStateType>
auto RegexASTGroup<NFAStateType>::complement(std::vector<Range> const& ranges)
        -> std::vector<typename RegexASTGroup<NFAStateType>::Range> {
    std::vector<Range> complemented;
    uint32_t low = 0;
    for (Range const& r : ranges) {
        if (r.first > 0) {
            complemented.emplace_back(low, r.first - 1);
        }
        low = r.second + 1;
    }
    if (low > 0) {
        complemented.emplace_back(low, cUnicodeMax);
    }
    return complemented;
}

template <typename NFAStateType>
void RegexASTGroup<NFAStateType>::add(RegexNFA<NFAStateType>* nfa, NFAStateType* end_state) {
    std::sort(this->m_ranges.begin(), this->m_ranges.end());
    std::vector<Range> merged = RegexASTGroup::merge(this->m_ranges);
    if (this->m_negate) {
        merged = RegexASTGroup::complement(merged);
    }
    for (Range const& r : merged) {
        nfa->get_root()->add_interval(Interval(r.first, r.second), end_state);
    }
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_AST_TPP
