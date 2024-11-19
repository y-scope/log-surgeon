#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP

#include <cstdint>
#include <memory>
#include <optional>
#include <queue>
#include <set>
#include <string>
#include <tuple>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

#include <fmt/core.h>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/RegexNFAState.hpp>
#include <log_surgeon/LexicalRule.hpp>

namespace log_surgeon::finite_automata {
// TODO: rename `RegexNFA` to `NFA`
template <typename NFAStateType>
class RegexNFA {
public:
    using StateVec = std::vector<NFAStateType*>;

    explicit RegexNFA(std::vector<LexicalRule<NFAStateType>> rules);

    /**
     * Creates a unique_ptr for an NFA state with no tagged transitions and adds it to `m_states`.
     * @return NFAStateType*
     */
    [[nodiscard]] auto new_state() -> NFAStateType*;

    /**
     * Creates a unique_ptr for an NFA state with a positive tagged end transition and adds it to
     * `m_states`.
     * @param tag
     * @param dest_state
     * @return NFAStateType*
     */
    [[nodiscard]] auto new_state_with_positive_tagged_end_transition(
            Tag const* tag,
            NFAStateType const* dest_state
    ) -> NFAStateType*;

    /**
     * Creates a unique_ptr for an NFA state with a negative tagged transition and adds it to
     * `m_states`.
     * @param tags
     * @param dest_state
     * @return NFAStateType*
     */
    [[nodiscard]] auto new_state_with_negative_tagged_transition(
            std::vector<Tag const*> tags,
            NFAStateType const* dest_state
    ) -> NFAStateType*;

    /**
     * Add two NFA states for a capture group:
     * 1. A start state: `m_root` --(start `tag`)--> start_state.
     * 2. An end state: end_state --(end `tag`)--> `dest_state`.
     * @param tag
     * @param dest_state
     * @return std::pair<NFAStateType*, NFAStateType*>
     */
    [[nodiscard]] auto new_capture_group_start_states(
            Tag const* tag,
            NFAStateType const* dest_state
    ) -> std::pair<NFAStateType*, NFAStateType*>;

    /**
     * @return A vector representing the traversal order of the NFA states using breadth-first
     * search (BFS).
     */
    [[nodiscard]] auto get_bfs_traversal_order() const -> std::vector<NFAStateType const*>;

    /**
     * @return A string representation of the NFA.
     */
    [[nodiscard]] auto serialize() const -> std::string;

    auto add_root_interval(Interval interval, NFAStateType* dest_state) -> void {
        m_root->add_interval(interval, dest_state);
    }

    auto set_root(NFAStateType* root) -> void { m_root = root; }

    auto get_root() -> NFAStateType* { return m_root; }

private:
    std::vector<std::unique_ptr<NFAStateType>> m_states;
    NFAStateType* m_root;
    // Store the rules locally as they contain information needed by the NFA. E.g., transitions in
    // the NFA point to tags in the rule ASTs.
    std::vector<LexicalRule<NFAStateType>> m_rules;
};

template <typename NFAStateType>
RegexNFA<NFAStateType>::RegexNFA(std::vector<LexicalRule<NFAStateType>> rules)
        : m_root{new_state()},
          m_rules{std::move(rules)} {
    for (auto const& rule : m_rules) {
        rule.add_to_nfa(this);
    }
}

template <typename NFAStateType>
auto RegexNFA<NFAStateType>::new_state() -> NFAStateType* {
    m_states.emplace_back(std::make_unique<NFAStateType>());
    return m_states.back().get();
}

template <typename NFAStateType>
auto RegexNFA<NFAStateType>::new_state_with_positive_tagged_end_transition(
        Tag const* tag,
        NFAStateType const* dest_state
) -> NFAStateType* {
    m_states.emplace_back(std::make_unique<NFAStateType>(tag, dest_state));
    return m_states.back().get();
}

template <typename NFAStateType>
auto RegexNFA<NFAStateType>::new_state_with_negative_tagged_transition(
        std::vector<Tag const*> tags,
        NFAStateType const* dest_state
) -> NFAStateType* {
    m_states.emplace_back(std::make_unique<NFAStateType>(std::move(tags), dest_state));
    return m_states.back().get();
}

template <typename NFAStateType>
auto RegexNFA<NFAStateType>::new_capture_group_start_states(
        Tag const* tag,
        NFAStateType const* dest_state
) -> std::pair<NFAStateType*, NFAStateType*> {
    auto* start_state = new_state();
    m_root->add_positive_tagged_start_transition(tag, start_state);

    auto* end_state = new_state_with_positive_tagged_end_transition(tag, dest_state);

    return {start_state, end_state};
}

template <typename NFAStateType>
auto RegexNFA<NFAStateType>::get_bfs_traversal_order() const -> std::vector<NFAStateType const*> {
    std::queue<NFAStateType const*> state_queue;
    std::unordered_set<NFAStateType const*> visited_states;
    std::vector<NFAStateType const*> visited_order;
    visited_states.reserve(m_states.size());
    visited_order.reserve(m_states.size());

    auto add_to_queue_and_visited
            = [&state_queue, &visited_states](NFAStateType const* dest_state) {
                  if (visited_states.insert(dest_state).second) {
                      state_queue.push(dest_state);
                  }
              };

    add_to_queue_and_visited(m_root);
    while (false == state_queue.empty()) {
        auto const* current_state = state_queue.front();
        visited_order.push_back(current_state);
        state_queue.pop();
        // TODO: handle the utf8 case
        for (uint32_t idx{0}; idx < cSizeOfByte; ++idx) {
            for (auto const* dest_state : current_state->get_byte_transitions(idx)) {
                add_to_queue_and_visited(dest_state);
            }
        }
        for (auto const* dest_state : current_state->get_epsilon_transitions()) {
            add_to_queue_and_visited(dest_state);
        }
        for (auto const& positive_tagged_start_transition :
             current_state->get_positive_tagged_start_transitions())
        {
            add_to_queue_and_visited(positive_tagged_start_transition.get_dest_state());
        }

        auto const& optional_positive_tagged_end_transition
                = current_state->get_positive_tagged_end_transitions();
        if (optional_positive_tagged_end_transition.has_value()) {
            add_to_queue_and_visited(optional_positive_tagged_end_transition.value().get_dest_state(
            ));
        }

        auto const& optional_negative_tagged_transition
                = current_state->get_negative_tagged_transition();
        if (optional_negative_tagged_transition.has_value()) {
            add_to_queue_and_visited(optional_negative_tagged_transition.value().get_dest_state());
        }
    }
    return visited_order;
}

template <typename NFAStateType>
auto RegexNFA<NFAStateType>::serialize() const -> std::string {
    auto const traversal_order = get_bfs_traversal_order();

    std::unordered_map<NFAStateType const*, uint32_t> state_ids;
    for (auto const* state : traversal_order) {
        state_ids.emplace(state, state_ids.size());
    }

    std::vector<std::string> serialized_states;
    for (auto const* state : traversal_order) {
        // `state_ids` is well-formed as its generated from `get_bfs_traversal_order` so we can
        // safely assume `state->serialize(state_ids)` will return a valid value.
        // NOLINTNEXTLINE(bugprone-unchecked-optional-access)
        serialized_states.emplace_back(state->serialize(state_ids).value());
    }
    return fmt::format("{}\n", fmt::join(serialized_states, "\n"));
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGEX_NFA_HPP
