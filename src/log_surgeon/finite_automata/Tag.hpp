#ifndef LOG_SURGEON_FINITE_AUTOMATA_TAG
#define LOG_SURGEON_FINITE_AUTOMATA_TAG

#include <cstdint>
#include <string>
#include <vector>

namespace log_surgeon::finite_automata {
struct Tag {
    std::string name;
    std::vector<uint32_t> starts;
    std::vector<uint32_t> ends;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_TAG
