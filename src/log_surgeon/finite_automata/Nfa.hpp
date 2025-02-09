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
#include <log_surgeon/UniqueIdGenerator.hpp>

namespace log_surgeon::finite_automata {
/**
 * Represents a Non-Deterministic Finite Automaton (NFA) designed to recognize a language based on
 * a set of rules provided during initialization. This class serves as an intermediate
 * representation used for generating the corresponding Deterministic Finite Automaton (DFA).
 *
 * NOTE: It is assumed that all capture groups have unique names, even across different rules.
 * @tparam TypedNfaState
 */
template <typename TypedNfaState>
class Nfa {
public:
    using StateVec = std::vector<TypedNfaState*>;

    explicit Nfa(std::vector<LexicalRule<TypedNfaState>> const& rules);

    /**
     * Creates a unique_ptr for an NFA state with no tagged transitions and adds it to `m_states`.
     * @return TypedNfaState*
     */
    [[nodiscard]] auto new_state() -> TypedNfaState*;

    /**
     * Creates a unique_ptr for an NFA state with a negative tagged transition and adds it to
     * `m_states`.
     * @param captures
     * @param dest_state
     * @return TypedNfaState*
     */
    [[nodiscard]] auto new_state_with_negative_tagged_transition(
            std::vector<Capture const*> const& captures,
            TypedNfaState const* dest_state
    ) -> TypedNfaState*;

    /**
     * Creates the start and end states for a capture group.
     * @param capture The capture associated with the capture group.
     * @param dest_state
     * @return A pair of states:
     * - A new state with a positive tagged start transition from `m_root`.
     * - A new state with a positive tagged end transition to `dest_state`.
     */
    [[nodiscard]] auto new_start_and_end_states_with_positive_tagged_transitions(
            Capture const* capture,
            TypedNfaState const* dest_state
    ) -> std::pair<TypedNfaState*, TypedNfaState*>;

    /**
     * @return A vector representing the traversal order of the NFA states using breadth-first
     * search (BFS).
     */
    [[nodiscard]] auto get_bfs_traversal_order() const -> std::vector<TypedNfaState const*>;

    /**
     * @return A string representation of the NFA.
     */
    [[nodiscard]] auto serialize() const -> std::string;

    auto add_root_interval(Interval interval, TypedNfaState* dest_state) -> void {
        m_root->add_interval(interval, dest_state);
    }

    auto set_root(TypedNfaState* root) -> void { m_root = root; }

    auto get_root() -> TypedNfaState* { return m_root; }

    [[nodiscard]] auto get_capture_to_tag_id_pair(
    ) const -> std::unordered_map<Capture const*, std::pair<tag_id_t, tag_id_t>> const& {
        return m_capture_to_tag_id_pair;
    }

private:
    /**
     * Creates start and end tags for the specified capture if they don't currently exist.
     * @param capture The variable to be captured.
     * @return A pair of tags:
     * - The start tag for the `capture`.
     * - The end tag for the `capture`.
     */
    [[nodiscard]] auto get_or_create_capture_tag_pair(Capture const* capture
    ) -> std::pair<tag_id_t, tag_id_t>;

    /**
     * Creates a `unique_ptr` for an NFA state with a positive tagged end transition and adds it to
     * `m_states`.
     * @param tag_id
     * @param dest_state
     * @return A new state with a positive tagged end transition to `dest_state`.
     */
    [[nodiscard]] auto new_state_with_positive_tagged_end_transition(
            tag_id_t tag_id,
            TypedNfaState const* dest_state
    ) -> TypedNfaState*;

    std::vector<std::unique_ptr<TypedNfaState>> m_states;
    // TODO: Lexer currently enforces unique naming across capture groups. However, this limits use
    // cases. Possibly initialize this in the lexer and pass it in during construction.
    std::unordered_map<Capture const*, std::pair<tag_id_t, tag_id_t>> m_capture_to_tag_id_pair;
    TypedNfaState* m_root;
    UniqueIdGenerator m_unique_id_generator;
};

template <typename TypedNfaState>
Nfa<TypedNfaState>::Nfa(std::vector<LexicalRule<TypedNfaState>> const& rules)
        : m_root{new_state()} {
    for (auto const& rule : rules) {
        rule.add_to_nfa(this);
    }
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::get_or_create_capture_tag_pair(Capture const* capture
) -> std::pair<tag_id_t, tag_id_t> {
    if (false == m_capture_to_tag_id_pair.contains(capture)) {
        auto const start_tag{m_unique_id_generator.generate_id()};
        auto const end_tag{m_unique_id_generator.generate_id()};
        m_capture_to_tag_id_pair.emplace(capture, std::make_pair(start_tag, end_tag));
    }
    return m_capture_to_tag_id_pair.at(capture);
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::new_state() -> TypedNfaState* {
    m_states.emplace_back(std::make_unique<TypedNfaState>());
    return m_states.back().get();
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::new_state_with_positive_tagged_end_transition(
        tag_id_t const tag_id,
        TypedNfaState const* dest_state
) -> TypedNfaState* {
    m_states.emplace_back(std::make_unique<TypedNfaState>(tag_id, dest_state));
    return m_states.back().get();
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::new_state_with_negative_tagged_transition(
        std::vector<Capture const*> const& captures,
        TypedNfaState const* dest_state
) -> TypedNfaState* {
    std::vector<tag_id_t> tags;
    for (auto const capture : captures) {
        auto const [start_tag, end_tag]{get_or_create_capture_tag_pair(capture)};
        tags.push_back(start_tag);
        tags.push_back(end_tag);
    }

    m_states.emplace_back(std::make_unique<TypedNfaState>(std::move(tags), dest_state));
    return m_states.back().get();
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::new_start_and_end_states_with_positive_tagged_transitions(
        Capture const* capture,
        TypedNfaState const* dest_state
) -> std::pair<TypedNfaState*, TypedNfaState*> {
    auto const [start_tag, end_tag]{get_or_create_capture_tag_pair(capture)};
    auto* start_state = new_state();
    m_root->add_positive_tagged_start_transition(start_tag, start_state);
    auto* end_state{new_state_with_positive_tagged_end_transition(end_tag, dest_state)};
    return {start_state, end_state};
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::get_bfs_traversal_order() const -> std::vector<TypedNfaState const*> {
    std::queue<TypedNfaState const*> state_queue;
    std::unordered_set<TypedNfaState const*> visited_states;
    std::vector<TypedNfaState const*> visited_order;
    visited_states.reserve(m_states.size());
    visited_order.reserve(m_states.size());

    auto add_to_queue_and_visited
            = [&state_queue, &visited_states](TypedNfaState const* dest_state) {
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
                = current_state->get_positive_tagged_end_transition();
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

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::serialize() const -> std::string {
    auto const traversal_order = get_bfs_traversal_order();

    std::unordered_map<TypedNfaState const*, uint32_t> state_ids;
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
