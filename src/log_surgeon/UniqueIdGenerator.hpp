#ifndef LOG_SURGEON_UNIQUEIDGENERATOR_HPP
#define LOG_SURGEON_UNIQUEIDGENERATOR_HPP

namespace log_surgeon {
class UniqueIdGenerator {
public:
    UniqueIdGenerator() : current_id{0} {}

    [[nodiscard]] auto generate_id() -> uint32_t { return current_id++; }

private:
    uint32_t current_id;
};
}  // namespace log_surgeon

#endif  // LOG_SURGEON_UNIQUEIDGENERATOR_HPP
