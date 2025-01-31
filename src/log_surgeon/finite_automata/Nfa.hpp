#ifndef LOG_SURGEON_FINITE_AUTOMATA_NFA_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_NFA_HPP

#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <queue>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <utility>
#include <vector>

#include <fmt/core.h>
#include <fmt/format.h>

#include <log_surgeon/Constants.hpp>
#include <log_surgeon/finite_automata/Capture.hpp>
#include <log_surgeon/finite_automata/TagOperation.hpp>
#include <log_surgeon/finite_automata/UnicodeIntervalTree.hpp>
#include <log_surgeon/LexicalRule.hpp>
#include <log_surgeon/UniqueIdGenerator.hpp>

namespace log_surgeon::finite_automata {
/**
 * Represents a NFA(non-deterministic finite automata) for recognizing a language based on the set
 * of rules used during initialization. Currently use as an intermediate model for generating the
 * DFA.
 *
 * Currently we assume all capture groups have unique names.
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
     * Creates a unique_ptr for an NFA state that is accepting and adds it to `m_states`.
     * @param matching_variable_id The variable id that the NFA state accepts.
     * @return TypedNfaState*
     */
    [[nodiscard]] auto new_accepting_state(uint32_t matching_variable_id) -> TypedNfaState*;

    /**
     * Creates a unique_ptr for an NFA state with a negative tagged transition and adds it to
     * `m_states`.
     * @param captures
     * @param dest_state
     * @return TypedNfaState*
     */
    [[nodiscard]] auto new_state_for_negative_captures(
            std::vector<Capture const*> const& captures,
            TypedNfaState const* dest_state
    ) -> TypedNfaState*;

    /**
     * Creates the start and end states for a capture group.
     * @param capture The capture associated with the capture group.
     * @param dest_state
     * @return A pair of states:
     * - A state from `m_root` with an outgoing transition that sets the start tag for the capture.
     * - A state with an outgoing transition to `dest_state` that sets the end tag for the capture.
     */
    [[nodiscard]] auto new_start_and_end_states_for_capture(
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
     * @return Forwards `NfaState::serialize`'s return value (std::nullopt) on failure.
     */
    [[nodiscard]] auto serialize() const -> std::optional<std::string>;

    auto add_root_interval(Interval interval, TypedNfaState* dest_state) -> void {
        m_root->add_interval(interval, dest_state);
    }

    auto set_root(TypedNfaState* root) -> void { m_root = root; }

    auto get_root() const -> TypedNfaState* { return m_root; }

    [[nodiscard]] auto get_num_tags() const -> size_t { return m_tag_id_generator.get_num_ids(); }

    [[nodiscard]] auto get_capture_to_tag_ids(
    ) const -> std::unordered_map<Capture const*, std::pair<tag_id_t, tag_id_t>> {
        return m_capture_to_tag_ids;
    }

private:
    /**
     * Creates start and end tags for the specified capture if they don't currently exist.
     * @param capture
     * @return The start and end tags corresponding to `capture`.
     */
    auto get_or_create_capture_tags(Capture const* capture) -> std::pair<tag_id_t, tag_id_t>;

    std::vector<std::unique_ptr<TypedNfaState>> m_states;
    // TODO: Lexer currently enforces unique naming across capture groups. However, this limits use
    // cases. Possibly initialize this in the lexer and pass it in during construction.
    std::unordered_map<Capture const*, std::pair<tag_id_t, tag_id_t>> m_capture_to_tag_ids;
    TypedNfaState* m_root;
    UniqueIdGenerator m_state_id_generator;
    UniqueIdGenerator m_tag_id_generator;
};

template <typename TypedNfaState>
Nfa<TypedNfaState>::Nfa(std::vector<LexicalRule<TypedNfaState>> const& rules)
        : m_root{new_state()} {
    for (auto const& rule : rules) {
        rule.add_to_nfa(this);
    }
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::get_or_create_capture_tags(Capture const* capture
) -> std::pair<tag_id_t, tag_id_t> {
    auto const existing_tags{m_capture_to_tag_ids.find(capture)};
    if (m_capture_to_tag_ids.end() == existing_tags) {
        auto start_tag{m_tag_id_generator.generate_id()};
        auto end_tag{m_tag_id_generator.generate_id()};
        auto new_tags{std::make_pair(start_tag, end_tag)};
        m_capture_to_tag_ids.emplace(capture, new_tags);
        return new_tags;
    }
    return existing_tags->second;
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::new_state() -> TypedNfaState* {
    m_states.emplace_back(std::make_unique<TypedNfaState>(m_state_id_generator.generate_id()));
    return m_states.back().get();
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::new_accepting_state(uint32_t const matching_variable_id
) -> TypedNfaState* {
    m_states.emplace_back(std::make_unique<TypedNfaState>(
            m_state_id_generator.generate_id(),
            matching_variable_id
    ));
    return m_states.back().get();
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::new_state_for_negative_captures(
        std::vector<Capture const*> const& captures,
        TypedNfaState const* dest_state
) -> TypedNfaState* {
    std::vector<tag_id_t> tags;
    for (auto const* capture : captures) {
        auto [start_tag, end_tag]{get_or_create_capture_tags(capture)};
        tags.push_back(start_tag);
        tags.push_back(end_tag);
    }

    m_states.emplace_back(std::make_unique<TypedNfaState>(
            m_state_id_generator.generate_id(),
            TagOperationType::Negate,
            std::move(tags),
            dest_state
    ));
    return m_states.back().get();
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::new_start_and_end_states_for_capture(
        Capture const* capture,
        TypedNfaState const* dest_state
) -> std::pair<TypedNfaState*, TypedNfaState*> {
    auto [start_tag, end_tag]{get_or_create_capture_tags(capture)};
    auto* start_state = new_state();
    m_root->add_spontaneous_transition(TagOperationType::Set, {start_tag}, start_state);
    m_states.emplace_back(std::make_unique<TypedNfaState>(
            m_state_id_generator.generate_id(),
            TagOperationType::Set,
            std::vector{end_tag},
            dest_state
    ));
    auto* end_state{m_states.back().get()};
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
        for (auto const& spontaneous_transition : current_state->get_spontaneous_transitions()) {
            add_to_queue_and_visited(spontaneous_transition.get_dest_state());
        }
    }
    return visited_order;
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::serialize() const -> std::optional<std::string> {
    auto const traversal_order = get_bfs_traversal_order();

    std::unordered_map<TypedNfaState const*, uint32_t> state_ids;
    for (auto const* state : traversal_order) {
        state_ids.emplace(state, state_ids.size());
    }

    std::vector<std::string> serialized_states;
    for (auto const* state : traversal_order) {
        auto const optional_serialized_state{state->serialize(state_ids)};
        if (false == optional_serialized_state.has_value()) {
            return std::nullopt;
        }
        serialized_states.emplace_back(optional_serialized_state.value());
    }
    return fmt::format("{}\n", fmt::join(serialized_states, "\n"));
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_NFA_HPP
