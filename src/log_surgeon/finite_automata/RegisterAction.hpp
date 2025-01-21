#ifndef LOG_SURGEON_FINITE_AUTOMATA_REGISTERACTION_HPP
#define LOG_SURGEON_FINITE_AUTOMATA_REGISTERACTION_HPP

#include <log_surgeon/finite_automata/Register.hpp>

namespace log_surgeon::finite_automata {
enum class RegisterOperation {
    AssignCurrentPosition,
    CopyOtherRegister
};

class RegisterAction {
public:
    RegisterAction(register_id_t const reg_id, RegisterOperation const reg_op)
            : m_reg_id{reg_id},
              m_reg_op{reg_op} {}

private:
    register_id_t m_reg_id;
    RegisterOperation m_reg_op;
};
}  // namespace log_surgeon::finite_automata

#endif  // LOG_SURGEON_FINITE_AUTOMATA_REGISTERACTION_HPP
