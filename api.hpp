#include <cstddef>
#include <cstdio>
#include <functional>
#include <map>
#include <optional>
#include <span>
#include <string_view>
#include <vector>

class Log;
class LogView;
class Schema;
typedef enum {
    ErrorCode_Success = 0,
    ErrorCode_BadParam,
    ErrorCode_errno,
    ErrorCode_NoAccess,
    ErrorCode_Failure,
} ErrorCode;

/**
 * Class allowing user to perform all reading and provide the parser with a
 * buffer containing the bytes to parse.
 */
class BufferParser {
  public:
    /**
     * Construct statically to more cleanly report errors building the parser
     * from the given schema. Can construct from from a file, or a Schema
     * object. CLP heuristic based schema file will provided in repository.
     */
    static std::optional<BufferParser> BufferParserFromFile(char const *schema_file);
    static std::optional<BufferParser> BufferParserFromSchema(Schema schema);

    /**
     * Attempts to parse the next log inside `buf`, up to `size`. The bytes
     * between `read_to` and `size` may contain a partial log message. It is
     * the user's responsibility to preserve these bytes and re-parse the log
     * message.
     * @param buf The byte buffer containing raw logs to be parsed
     * @param size The size of `buf`
     * @param read_to Populated with the number of bytes parsed from `buf`. If
     * no log was parsed it will be 0.
     * @param logview Populated with the next log parsed from `buf`
     * @param end Treat the end of the buffer as the end of input to parse the
     * final log message.
     * @return ErrorCode enum (0 on success)
     */
    ErrorCode getNextLogView(char *buf, size_t size, size_t &read_to, LogView &logview,
                             bool end = false);
    /**
     * Attempts to parse the next N logs inside `buf`, up to `size`. The bytes
     * between `read_to` and `size` may contain a partial log message. It is
     * the user's responsibility to preserve these bytes and re-parse the log
     * message.
     * @param buf The byte buffer containing raw logs to be parsed
     * @param size The size of `buf`
     * @param read_to Populated with the number of bytes read from `buf`
     * @param logviews Vector populated with the next parsed logs from `buf`
     * @param count The number of logs to attempt to parse; defaults to 0,
     * which reads as many logs as possible
     * @param end Treat the end of the buffer as the end of input to parse the
     * final log message.
     * @return ErrorCode enum (0 on success)
     */
    ErrorCode getNLogViews(char *buf, size_t size, size_t &read_to, std::vector<LogView> &logviews,
                           size_t count = 0, bool end = false);

    // if c++20 can replace 'char* buf, size_t size' with 'std::span<char> buf'
    ErrorCode getNextLogView(std::span<char> buf, size_t &read_to, LogView &logview,
                             bool end = false);
    ErrorCode getNLogViews(std::span<char> buf, size_t &readto, std::vector<LogView> &logviews,
                           size_t count = 0, bool end = false);
};

/**
 * Class providing the parser with the source to read from allowing the parser
 * to perform any reading as necessary.
 */
class FileParser {
  public:
    /**
     * Construct statically to more cleanly report errors building the parser
     * from the given schema. Can construct from from a file, or a Schema
     * object. CLP heuristic based schema file will provided in repository.
     */
    static std::optional<FileParser> FileParserFromFile(char const *schema_file,
                                                        char const *log_file);
    static std::optional<FileParser> FileParserFromSchema(Schema schema, char const *log_file);

    /**
     * Attempts to parse the next log from the file the parser was created
     * with.
     * @param logview Populated with the next log parsed from the file
     * @return ErrorCode enum (0 on success)
     */
    ErrorCode getNextLogView(LogView &logview);
    /**
     * Attempts to parse the next N logs from the file the parser was created
     * with.
     * @param logviews Vector populated with the next parsed logs from the file
     * @param count The number of logs to attempt to parse; defaults to 0,
     * which reads as many logs as possible
     * @return ErrorCode enum (0 on success)
     */
    ErrorCode getNLogViews(std::vector<LogView> &logviews, size_t count = 0);
};

/**
 * Minimal interface necessary for the parser to invoke reading as necessary.
 * Allowing the parser to invoke read helps users avoid unnecessary copying,
 * makes the lifetime of LogViews easier to understand, and makes the user code
 * cleaner.
 */
class Reader {
  public:
    /**
     * Function to read from some unknown source into specified destination buffer
     * @param char* Destination byte buffer to read into
     * @param size_t Amount to read up to
     * @return The amount read
     */
    std::function<size_t(char *, size_t)> read;
    /**
     * @return True if the source has been exhausted (e.g. EOF reached)
     */
    std::function<bool()> done;
};

/**
 * Class providing the parser with the source to read from allowing the parser
 * to perform any reading as necessary.
 */
class ReaderParser {
  public:
  public:
    /**
     * Construct statically to more cleanly report errors building the parser
     * from the given schema. Can construct from from a file, or a Schema
     * object. CLP heuristic based schema file will provided in repository.
     */
    static std::optional<ReaderParser> ReaderParserFromFile(char const *schema_file, Reader &r);
    static std::optional<ReaderParser> ReaderParserFromSchema(Schema schema, Reader &r);

    /**
     * Attempts to parse the next log from the reader the parser was created
     * with.
     * @param logview Populated with the next log parsed from the reader
     * @return ErrorCode enum (0 on success)
     */
    ErrorCode getNextLogView(LogView &logview);
    /**
     * Attempts to parse the next N logs from the reader the parser was created
     * with.
     * @param logviews Vector populated with the next parsed logs from the
     * reader
     * @param count The number of logs to attempt to parse; defaults to 0,
     * which reads as many logs as possible
     * @return ErrorCode enum (0 on success)
     */
    ErrorCode getNLogViews(std::vector<LogView> &logviews, size_t count = 0);
};

/**
 * An object that represents a parsed log.
 * Contains ways to access parsed variables and information from the original
 * raw log.
 * All returned string_views point into the original source buffer used to
 * parse the log. Thus, the components of a LogView are weak references to the
 * original buffer, and will become undefined if they exceed the lifetime of
 * the original buffer or the original buffer is mutated.
 */
class LogView {
  public:
    // Likely to only be used by the parser itself.
    LogView();

    /**
     * Copy the tokens representing a log out of the source buffer by iterating them.
     * This allows the returned Log to own all its tokens.
     * @return A Log object made from this LogView.
     */
    Log deepCopy();

    /**
     * @param var_id The occurrence of the variable in the log starting at 0.
     * @param occurrence The occurrence of the variable in the log starting at 0.
     * @return View of the variable from the source buffer.
     */
    std::string_view getVarByName(std::string_view var_name, size_t occurrence);
    // Convenience functions for common variables
    std::string_view getVerbosity() { return getVarByName("verbosity", 0); };
    std::string_view getTimestamp() { return getVarByName("timestamp", 0); };

    // Use the variable ID rather than its name. Meant for internal use, but
    // does save a lookup to map the string name to its id.
    std::string_view getVarByID(size_t var_id, size_t occurrence);

    /**
     * @return The timestamp encoded as the time in ms since unix epoch.
     */
    uint64_t getEpochTimestampMs();

    /**
     * The parser considers the start of a log to be denoted by a new line
     * character followed by a timestamp (other than for the first log of a
     * file).
     * A log is considered to contain multiple lines if at least one new line
     * character is consumed by the parser before finding the start of the next
     * log or exhausting the source (e.g. EOF).
     * @return True if the log spans multiple lines.
     */
    bool isMultiLine();

    /**
     * Reconstructs the raw log it represents by iterating the log's tokens and
     * copying the contents of each into a string (similar to deepCopy).
     * @return A reconstructed raw log.
     */
    std::string getLog();

    /**
     * Constructs a user friendly/readable representation of the log's log
     * type. A log type is essentially the static text of a log with the
     * variable components replaced with their name/id. Therefore, two separate
     * log messages from the same logging source code will have the same log
     * type.
     * @return The log type of this log.
     */
    std::string getLogType();

  private:
    bool multiline;
    // placeholder for internal data structure: essentially an array of tokens
};

/**
 * Contains all of the data necessary to form the log.
 * Essentially replaces the source buffer originally used by the parser.
 * On construction string_views will now point to byte_array rather than
 * the original source buffer.
 */
class Log : public LogView {
  public:
    // Equivalent to LogView::deepCopy
    Log(LogView src);

  private:
    char *byte_array;
};

/**
 * Schema class Copied from existing code.
 * Contains various functions to manipulate a schema programmatically. The
 * majority of use cases should not require users to modify the schema
 * programmatically, allowing them to simply edit their schema file.
 */
class Schema {
  public:
    Schema() {}
    Schema(std::FILE *schema_file);
    Schema(std::string schema_string);

    void load_from_file(std::FILE *schema_file);
    void load_from_string(std::string schema_string);
    void add_variable(std::string var_name, std::string regex);
    void remove_variable(std::string var_name);
    void add_variables(std::map<std::string, std::string> variables);
    void remove_variables(std::map<std::string, std::string> variables);
    void remove_all_variables();
    void set_variables(std::map<std::string, std::string> variables);
    void add_delimiter(char delimiter);
    void remove_delimiter(char delimiter);
    void add_delimiters(std::vector<char> delimiter);
    void remove_delimiters(std::vector<char> delimiter);
    void remove_all_delimiters();
    void set_delimiters(std::vector<char> delimiters);
    void clear();

  private:
    std::vector<char> m_delimiters;
    std::map<std::string, std::string> m_variables;
};
