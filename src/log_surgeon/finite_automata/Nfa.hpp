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
#include <log_surgeon/types.hpp>
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
     * @return A pointer to the newly created NFA state with no spontaneous transitions.
     */
    [[nodiscard]] auto new_state() -> TypedNfaState*;

    /*
     * @param matching_variable_id The id for the variable matched by this state.
     * @return A pointer to the newly created accepting NFA state.
     */
    [[nodiscard]] auto new_accepting_state(uint32_t matching_variable_id) -> TypedNfaState*;

    /**
     * @param captures A vector containing the captures of all alternate paths.
     * @param dest_state The destination state to arrive at after negating the captures.
     * @param multi_valued If the tag needs to store multiple positions to track the capture.
     * @return A pointer to the newly created NFA state with a spontaneous transition to
     * `dest_state`negating all the tags associated with `captures`.
     */
    [[nodiscard]] auto new_state_from_negative_captures(
            std::vector<Capture const*> const& captures,
            TypedNfaState const* dest_state,
            bool multi_valued
    ) -> TypedNfaState*;

    /**
     * @param capture The positive capture to be tracked.
     * @param dest_state The destination state to arrive at after tracking the capture.
     * @param multi_valued If the tag needs to store multiple positions to track the capture.
     * @return A pair of pointers to the two newly created NFA states:
     * - A state arrived at from a spontaneous transition out of `m_root` that sets a tag to track
     * the capture's start position.
     * - A state with a spontaneous transition to `dest_state` that sets a tag to track the
     * capture's end position
     */
    [[nodiscard]] auto new_start_and_end_states_from_positive_capture(
            Capture const* capture,
            TypedNfaState const* dest_state,
            bool multi_valued
    ) -> std::pair<TypedNfaState*, TypedNfaState*>;

    /**
     * @return A vector representing the traversal order of the NFA states using breadth-first
     * search (BFS).
     */
    [[nodiscard]] auto get_bfs_traversal_order() const -> std::vector<TypedNfaState const*>;

    /**
     * @return A string representation of the NFA on success.
     * @return Forwards `NfaState::serialize`'s return value (`std::nullopt`) on failure.
     */
    [[nodiscard]] auto serialize() const -> std::optional<std::string>;

    auto add_root_interval(Interval interval, TypedNfaState* dest_state) -> void {
        m_root->add_interval(interval, dest_state);
    }

    auto set_root(TypedNfaState* root) -> void { m_root = root; }

    auto get_root() const -> TypedNfaState* { return m_root; }

    [[nodiscard]] auto get_num_tags() const -> uint32_t { return m_tag_id_generator.get_num_ids(); }

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

    std::vector<std::unique_ptr<TypedNfaState>> m_states;
    // TODO: Lexer currently enforces unique naming across capture groups. However, this limits use
    // cases. Possibly initialize this in the lexer and pass it in during construction.
    std::unordered_map<Capture const*, std::pair<tag_id_t, tag_id_t>> m_capture_to_tag_id_pair;
    TypedNfaState* m_root;
    UniqueIdGenerator m_state_id_generator;
    UniqueIdGenerator m_tag_id_generator;
};

template <typename TypedNfaState>
Nfa<TypedNfaState>::Nfa(std::vector<LexicalRule<TypedNfaState>> const& rules) {
    m_root = new_state();
    for (auto const& rule : rules) {
        rule.add_to_nfa(this);
    }
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::get_or_create_capture_tag_pair(Capture const* capture
) -> std::pair<tag_id_t, tag_id_t> {
    if (false == m_capture_to_tag_id_pair.contains(capture)) {
        auto const start_tag{m_tag_id_generator.generate_id()};
        auto const end_tag{m_tag_id_generator.generate_id()};
        m_capture_to_tag_id_pair.emplace(capture, std::make_pair(start_tag, end_tag));
    }
    return m_capture_to_tag_id_pair.at(capture);
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
auto Nfa<TypedNfaState>::new_state_from_negative_captures(
        std::vector<Capture const*> const& captures,
        TypedNfaState const* dest_state,
        bool const multi_valued
) -> TypedNfaState* {
    std::vector<tag_id_t> tags;
    for (auto const* capture : captures) {
        auto const [start_tag, end_tag]{get_or_create_capture_tag_pair(capture)};
        tags.push_back(start_tag);
        tags.push_back(end_tag);
    }

    m_states.emplace_back(std::make_unique<TypedNfaState>(
            m_state_id_generator.generate_id(),
            TagOperationType::Negate,
            std::move(tags),
            dest_state,
            multi_valued
    ));
    return m_states.back().get();
}

template <typename TypedNfaState>
auto Nfa<TypedNfaState>::new_start_and_end_states_from_positive_capture(
        Capture const* capture,
        TypedNfaState const* dest_state,
        bool const multi_valued
) -> std::pair<TypedNfaState*, TypedNfaState*> {
    auto const [start_tag, end_tag]{get_or_create_capture_tag_pair(capture)};
    auto* start_state{new_state()};
    m_root->add_spontaneous_transition(
            TagOperationType::Set,
            {start_tag},
            start_state,
            multi_valued
    );
    m_states.emplace_back(std::make_unique<TypedNfaState>(
            m_state_id_generator.generate_id(),
            TagOperationType::Set,
            std::vector{end_tag},
            dest_state,
            multi_valued
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
