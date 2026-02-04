#ifndef LOG_SURGEON_FINITE_AUTOMATA_NFA_STATE_PAIR
#define LOG_SURGEON_FINITE_AUTOMATA_NFA_STATE_PAIR

#include <cstdint>
#include <set>
#include <vector>

#include <log_surgeon/Constants.hpp>

namespace log_surgeon::finite_automata {
/**
 * Class for a pair of NFA states, where each state in the pair belongs to a different NFA.
 * This class is used to facilitate the construction of an intersection NFA from two separate NFAs.
 * Each instance represents a state in the intersection NFA and follows these rules:
 *
 * - A pair is considered accepting if both states are accepting in their respective NFAs.
 * - A pair is considered reachable if both its states are reachable in their respective NFAs
 *   from this pair's states.
 *
 * NOTE: Only the first state in the pair contains the variable types matched by the pair.
 */
template <typename TypedNfaState>
class NfaStatePair {
public:
    NfaStatePair(TypedNfaState const* state1, TypedNfaState const* state2)
            : m_state1(state1),
              m_state2(state2) {}

    /**
     * @param rhs
     * @return Whether `m_state1` in lhs has a lower address than in rhs, or if they're equal,
     * whether `m_state2` in lhs has a lower address than in rhs.
     */
    auto operator<(NfaStatePair const& rhs) const -> bool {
        if (m_state1 == rhs.m_state1) {
            return m_state2 < rhs.m_state2;
        }
        return m_state1 < rhs.m_state1;
    }

    /**
     * Generates all pairs reachable from the current pair via any string and store any reachable
     * pair not previously visited in `unvisited_pairs`.
     * @param visited_pairs Previously visited pairs.
     * @param unvisited_pairs Set to add unvisited reachable pairs.
     */
    auto get_reachable_pairs(
            std::set<NfaStatePair>& visited_pairs,
            std::set<NfaStatePair>& unvisited_pairs
    ) const -> void;

    [[nodiscard]] auto is_accepting() const -> bool {
        return m_state1->is_accepting() && m_state2->is_accepting();
    }

    [[nodiscard]] auto get_matching_variable_id() const -> uint32_t {
        return m_state1->get_matching_variable_id();
    }

private:
    TypedNfaState const* m_state1;
    TypedNfaState const* m_state2;
};

template <typename TypedNfaState>
auto NfaStatePair<TypedNfaState>::get_reachable_pairs(
        std::set<NfaStatePair>& visited_pairs,
        std::set<NfaStatePair>& unvisited_pairs
) const -> void {
    // TODO: Handle UTF-8 (multi-byte transitions) as well
    for (uint32_t i{0}; i < cSizeOfByte; ++i) {
        auto const& byte_transitions1{m_state1->get_byte_transitions(i)};
        auto const& byte_transitions2{m_state2->get_byte_transitions(i)};
        for (auto const& dest1 : byte_transitions1) {
            for (auto const& dest2 : byte_transitions2) {
                NfaStatePair const reachable_pair{dest1, dest2};
                if (false == visited_pairs.contains(reachable_pair)) {
                    unvisited_pairs.insert(reachable_pair);
                }
            }
        }
    }
    auto const& epsilon_transitions1{m_state1->get_spontaneous_transitions()};
    for (auto const& transition1 : epsilon_transitions1) {
        auto const& dest1{transition1.get_dest_state()};
        NfaStatePair const reachable_pair{dest1, m_state2};
        if (false == visited_pairs.contains(reachable_pair)) {
            unvisited_pairs.insert(reachable_pair);
        }
    }
    auto const& epsilon_transitions2{m_state2->get_spontaneous_transitions()};
    for (auto const& transition2 : epsilon_transitions2) {
        auto const& dest2{transition2.get_dest_state()};
        NfaStatePair const reachable_pair{m_state1, dest2};
        if (false == visited_pairs.contains(reachable_pair)) {
            unvisited_pairs.insert(reachable_pair);
        }
    }
}
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_NFA_STATE_PAIR
