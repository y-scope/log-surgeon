#ifndef LOG_SURGEON_FINITE_AUTOMATA_NFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_NFA_HPP

#include <cstdint>
#include <memory>
#include <queue>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

#include <fmt/core.h>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/NfaState.hpp>
#include <log_surgeon/LexicalRule.hpp>

namespace log_surgeon::finite_automata {
template <typename NfaStateType>
class Nfa {
public:
    using StateVec = std::vector<NfaStateType*>;

    explicit Nfa(std::vector<LexicalRule<NfaStateType>> rules);

    /**
     * Creates a unique_ptr for an NFA state with no tagged transitions and adds it to `m_states`.
     * @return NfaStateType*
     */
    [[nodiscard]] auto new_state() -> NfaStateType*;

    /**
     * Creates a unique_ptr for an NFA state with a positive tagged transition and adds it to
     * `m_states`.
     * @param tag
     * @param dest_state
     * @return NfaStateType*
     */
    [[nodiscard]] auto new_state_with_positive_tagged_transition(
            Tag* tag,
            NfaStateType const* dest_state
    ) -> NfaStateType*;

    /**
     * Creates a unique_ptr for an NFA state with a negative tagged transition and adds it to
     * `m_states`.
     * @param tags
     * @param dest_state
     * @return NfaStateType*
     */
    [[nodiscard]] auto new_state_with_negative_tagged_transition(
            std::vector<Tag*> tags,
            NfaStateType const* dest_state
    ) -> NfaStateType*;

    /**
     * @return A vector representing the traversal order of the NFA states using breadth-first
     * search (BFS).
     */
    [[nodiscard]] auto get_bfs_traversal_order() const -> std::vector<NfaStateType const*>;

    /**
     * @return A string representation of the NFA.
     */
    [[nodiscard]] auto serialize() const -> std::string;

    auto add_root_interval(Interval interval, NfaStateType* dest_state) -> void {
        m_root->add_interval(interval, dest_state);
    }

    auto set_root(NfaStateType* root) -> void { m_root = root; }

    auto get_root() -> NfaStateType* { return m_root; }

private:
    std::vector<std::unique_ptr<NfaStateType>> m_states;
    NfaStateType* m_root;
    // Store the rules locally as they contain information needed by the NFA. E.g., transitions in
    // the NFA point to tags in the rule ASTs.
    std::vector<LexicalRule<NfaStateType>> m_rules;
};

template <typename NfaStateType>
Nfa<NfaStateType>::Nfa(std::vector<LexicalRule<NfaStateType>> rules)
        : m_root{new_state()},
          m_rules{std::move(rules)} {
    for (auto const& rule : m_rules) {
        rule.add_to_nfa(this);
    }
}

template <typename NfaStateType>
auto Nfa<NfaStateType>::new_state() -> NfaStateType* {
    m_states.emplace_back(std::make_unique<NfaStateType>());
    return m_states.back().get();
}

template <typename NfaStateType>
auto Nfa<NfaStateType>::new_state_with_positive_tagged_transition(
        Tag* tag,
        NfaStateType const* dest_state
) -> NfaStateType* {
    m_states.emplace_back(std::make_unique<NfaStateType>(tag, dest_state));
    return m_states.back().get();
}

template <typename NfaStateType>
auto Nfa<NfaStateType>::new_state_with_negative_tagged_transition(
        std::vector<Tag*> tags,
        NfaStateType const* dest_state
) -> NfaStateType* {
    m_states.emplace_back(std::make_unique<NfaStateType>(std::move(tags), dest_state));
    return m_states.back().get();
}

template <typename NfaStateType>
auto Nfa<NfaStateType>::get_bfs_traversal_order() const -> std::vector<NfaStateType const*> {
    std::queue<NfaStateType const*> state_queue;
    std::unordered_set<NfaStateType const*> visited_states;
    std::vector<NfaStateType const*> visited_order;
    visited_states.reserve(m_states.size());
    visited_order.reserve(m_states.size());

    auto add_to_queue_and_visited
            = [&state_queue, &visited_states](NfaStateType const* dest_state) {
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

template <typename NfaStateType>
auto Nfa<NfaStateType>::serialize() const -> std::string {
    auto const traversal_order = get_bfs_traversal_order();

    std::unordered_map<NfaStateType const*, uint32_t> state_ids;
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

#endif  // LOG_SURGEON_FINITE_AUTOMATA_NFA_HPP
