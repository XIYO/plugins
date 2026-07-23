import CoreGraphics
import Darwin
@preconcurrency import EventKit
import Foundation

private let calendarAdapterVersion = "0.1.2"

private enum CalendarAdapterError: Error, CustomStringConvertible {
    case usage(String)
    case operation(String)

    var description: String {
        switch self {
        case .usage(let message), .operation(let message):
            return message
        }
    }
}

private enum LogLevel: Int, Comparable {
    case debug = 10
    case info = 20
    case warn = 30
    case error = 40

    static func < (lhs: LogLevel, rhs: LogLevel) -> Bool {
        lhs.rawValue < rhs.rawValue
    }

    static func parse(_ value: String?) -> LogLevel {
        switch value?.lowercased() {
        case "debug": return .debug
        case "info": return .info
        case "error": return .error
        default: return .warn
        }
    }
}

private enum Logger {
    static let threshold = LogLevel.parse(ProcessInfo.processInfo.environment["SHERPA_CALENDAR_LOG_LEVEL"])

    static func write(_ level: LogLevel, _ message: String) {
        guard level >= threshold else { return }
        let label: String
        switch level {
        case .debug: label = "DEBUG"
        case .info: label = "INFO"
        case .warn: label = "WARN"
        case .error: label = "ERROR"
        }
        FileHandle.standardError.write(Data("\(label) \(message)\n".utf8))
    }
}

private struct Arguments {
    private(set) var values: [String]

    init(_ values: [String]) {
        self.values = values
    }

    mutating func flag(_ name: String) -> Bool {
        guard let index = values.firstIndex(of: name) else { return false }
        values.remove(at: index)
        return true
    }

    mutating func option(_ name: String) throws -> String? {
        guard let index = values.firstIndex(of: name) else { return nil }
        let valueIndex = values.index(after: index)
        guard valueIndex < values.endIndex else {
            throw CalendarAdapterError.usage("\(name) requires a value")
        }
        let value = values[valueIndex]
        values.remove(at: valueIndex)
        values.remove(at: index)
        return value
    }

    mutating func positional(_ label: String) throws -> String {
        guard let index = values.firstIndex(where: { !$0.hasPrefix("--") }) else {
            throw CalendarAdapterError.usage("missing \(label)")
        }
        return values.remove(at: index)
    }

    func finish() throws {
        guard values.isEmpty else {
            throw CalendarAdapterError.usage("unknown arguments: \(values.joined(separator: " "))")
        }
    }
}

private struct Output {
    let json: Bool

    func object(_ value: Any) throws {
        if json {
            let data = try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys])
            guard let text = String(data: data, encoding: .utf8) else {
                throw CalendarAdapterError.operation("failed to encode JSON output")
            }
            print(text)
        } else if let dictionary = value as? [String: Any] {
            printDictionary(dictionary)
        } else {
            print(value)
        }
    }

    func objects(_ values: [[String: Any]]) throws {
        if json {
            try object(values)
            return
        }
        if values.isEmpty {
            print("No results")
            return
        }
        for value in values {
            printDictionary(value)
            print("")
        }
    }

    private func printDictionary(_ value: [String: Any]) {
        for key in value.keys.sorted() {
            let item = value[key]!
            if item is NSNull { continue }
            if let array = item as? [Any] {
                print("\(key): \(array.map { String(describing: $0) }.joined(separator: ", "))")
            } else {
                print("\(key): \(item)")
            }
        }
    }
}

private enum Dates {
    static let isoOutput: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        formatter.timeZone = .current
        return formatter
    }()

    static func day(_ value: String, endOfDay: Bool = false) throws -> Date {
        let parts = value.split(separator: "-")
        guard parts.count == 3,
              let year = Int(parts[0]),
              let month = Int(parts[1]),
              let day = Int(parts[2]) else {
            throw CalendarAdapterError.usage("invalid date '\(value)'; expected YYYY-MM-DD")
        }
        var components = DateComponents()
        components.calendar = Calendar(identifier: .gregorian)
        components.timeZone = .current
        components.year = year
        components.month = month
        components.day = day
        if endOfDay {
            components.hour = 23
            components.minute = 59
            components.second = 59
        } else {
            components.hour = 0
            components.minute = 0
            components.second = 0
        }
        guard let date = components.date else {
            throw CalendarAdapterError.usage("invalid calendar date '\(value)'")
        }
        return date
    }

    static func dateTime(_ value: String) throws -> Date {
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = iso.date(from: value) { return date }
        iso.formatOptions = [.withInternetDateTime]
        if let date = iso.date(from: value) { return date }

        for pattern in ["yyyy-MM-dd'T'HH:mm:ss", "yyyy-MM-dd'T'HH:mm", "yyyy-MM-dd HH:mm"] {
            let formatter = DateFormatter()
            formatter.calendar = Calendar(identifier: .gregorian)
            formatter.locale = Locale(identifier: "en_US_POSIX")
            formatter.timeZone = .current
            formatter.isLenient = false
            formatter.dateFormat = pattern
            if let date = formatter.date(from: value) { return date }
        }
        throw CalendarAdapterError.usage("invalid date-time '\(value)'; use YYYY-MM-DDTHH:mm or ISO 8601")
    }

    static func string(_ date: Date?) -> Any {
        guard let date else { return NSNull() }
        return isoOutput.string(from: date)
    }
}

private final class CalendarStore {
    let store = EKEventStore()

    func authorizationName() -> String {
        let status = EKEventStore.authorizationStatus(for: .event)
        if status == .fullAccess { return "full-access" }
        if status == .writeOnly { return "write-only" }
        if status == .notDetermined { return "not-determined" }
        if status == .denied { return "denied" }
        if status == .restricted { return "restricted" }
        return "legacy-or-unknown"
    }

    func requireFullAccess() throws {
        let status = authorizationName()
        guard status == "full-access" else {
            if status == "not-determined" {
                throw CalendarAdapterError.operation("calendar access is not determined; run 'sherpa planner calendar authorize'")
            }
            if status == "write-only" {
                throw CalendarAdapterError.operation("full calendar access is required; run 'sherpa planner calendar authorize'")
            }
            throw CalendarAdapterError.operation("full calendar access is unavailable (\(status)); check System Settings > Privacy & Security > Calendars")
        }
    }

    func authorize() async throws -> Bool {
        Logger.write(.info, "[eventkit:authorize:start] Requesting full calendar access")
        let granted = try await store.requestFullAccessToEvents()
        Logger.write(.info, "[eventkit:authorize:success] Calendar access request completed")
        return granted
    }

    func sources() -> [EKSource] {
        store.sources.sorted {
            if $0.title == $1.title { return $0.sourceIdentifier < $1.sourceIdentifier }
            return $0.title.localizedCaseInsensitiveCompare($1.title) == .orderedAscending
        }
    }

    func calendars() -> [EKCalendar] {
        store.calendars(for: .event).sorted {
            if $0.source.title == $1.source.title {
                return $0.title.localizedCaseInsensitiveCompare($1.title) == .orderedAscending
            }
            return $0.source.title.localizedCaseInsensitiveCompare($1.source.title) == .orderedAscending
        }
    }

    func source(_ selector: String) throws -> EKSource {
        let all = sources()
        if let exactID = all.first(where: { $0.sourceIdentifier == selector }) { return exactID }
        let exact = all.filter { $0.title.caseInsensitiveCompare(selector) == .orderedSame }
        if exact.count == 1 { return exact[0] }
        let exactWithCalendars = exact.filter { !$0.calendars(for: .event).isEmpty }
        if exactWithCalendars.count == 1 { return exactWithCalendars[0] }
        let contains = all.filter { $0.title.localizedCaseInsensitiveContains(selector) }
        if contains.count == 1 { return contains[0] }
        let containsWithCalendars = contains.filter { !$0.calendars(for: .event).isEmpty }
        if containsWithCalendars.count == 1 { return containsWithCalendars[0] }
        if exact.count > 1 || contains.count > 1 {
            throw CalendarAdapterError.operation("source '\(selector)' is ambiguous; use a source ID from 'sherpa planner calendar sources'")
        }
        throw CalendarAdapterError.operation("source '\(selector)' not found; run 'sherpa planner calendar sources'")
    }

    func calendar(_ selector: String, sourceSelector: String?, writable: Bool = false) throws -> EKCalendar {
        var matches = calendars().filter {
            $0.calendarIdentifier == selector || $0.title.caseInsensitiveCompare(selector) == .orderedSame
        }
        if let sourceSelector {
            let selectedSource = try source(sourceSelector)
            matches = matches.filter { $0.source.sourceIdentifier == selectedSource.sourceIdentifier }
        }
        if writable {
            matches = matches.filter { $0.allowsContentModifications && !$0.isImmutable }
        }
        if matches.count == 1 { return matches[0] }
        if matches.isEmpty {
            throw CalendarAdapterError.operation("calendar '\(selector)' not found or not writable; run 'sherpa planner calendar calendars'")
        }
        throw CalendarAdapterError.operation("calendar '\(selector)' is ambiguous; add --source or use a calendar ID")
    }

    func event(_ identifier: String) throws -> EKEvent {
        guard let event = store.event(withIdentifier: identifier) else {
            throw CalendarAdapterError.operation("event '\(identifier)' not found; search again because occurrence IDs can change after edits")
        }
        return event
    }
}

private func sourceTypeName(_ type: EKSourceType) -> String {
    if type == .local { return "local" }
    if type == .exchange { return "exchange" }
    if type == .calDAV { return "caldav" }
    if type == .subscribed { return "subscribed" }
    if type == .birthdays { return "birthdays" }
    return "other-\(type.rawValue)"
}

private func color(_ cgColor: CGColor?) -> Any {
    guard let components = cgColor?.components, components.count >= 3 else { return NSNull() }
    let red = Int((components[0] * 255).rounded())
    let green = Int((components[1] * 255).rounded())
    let blue = Int((components[2] * 255).rounded())
    return String(format: "#%02X%02X%02X", red, green, blue)
}

private func parseColor(_ value: String) throws -> CGColor {
    let hex = value.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
    guard hex.count == 6, let number = UInt64(hex, radix: 16) else {
        throw CalendarAdapterError.usage("invalid color '\(value)'; expected #RRGGBB")
    }
    return CGColor(
        red: CGFloat((number >> 16) & 0xFF) / 255,
        green: CGFloat((number >> 8) & 0xFF) / 255,
        blue: CGFloat(number & 0xFF) / 255,
        alpha: 1
    )
}

private func sourceObject(_ source: EKSource) -> [String: Any] {
    [
        "id": source.sourceIdentifier,
        "title": source.title,
        "type": sourceTypeName(source.sourceType),
        "event_calendar_count": source.calendars(for: .event).count
    ]
}

private func calendarObject(_ calendar: EKCalendar) -> [String: Any] {
    [
        "id": calendar.calendarIdentifier,
        "title": calendar.title,
        "source": calendar.source.title,
        "source_id": calendar.source.sourceIdentifier,
        "source_type": sourceTypeName(calendar.source.sourceType),
        "writable": calendar.allowsContentModifications && !calendar.isImmutable,
        "color": color(calendar.cgColor)
    ]
}

private func recurrenceObject(_ event: EKEvent) -> [[String: Any]] {
    (event.recurrenceRules ?? []).map { rule in
        let frequency: String
        if rule.frequency == .daily { frequency = "daily" }
        else if rule.frequency == .weekly { frequency = "weekly" }
        else if rule.frequency == .monthly { frequency = "monthly" }
        else if rule.frequency == .yearly { frequency = "yearly" }
        else { frequency = "unknown" }
        return [
            "frequency": frequency,
            "interval": rule.interval,
            "end_date": Dates.string(rule.recurrenceEnd?.endDate),
            "occurrence_count": rule.recurrenceEnd?.occurrenceCount ?? 0
        ]
    }
}

private func eventObject(_ event: EKEvent, includeNotes: Bool = false) -> [String: Any] {
    var value: [String: Any] = [
        "event_id": event.eventIdentifier ?? NSNull(),
        "calendar_item_id": event.calendarItemIdentifier,
        "title": event.title ?? "",
        "calendar": event.calendar.title,
        "source": event.calendar.source.title,
        "start": Dates.string(event.startDate),
        "end": Dates.string(event.endDate),
        "all_day": event.isAllDay,
        "location": event.location ?? NSNull(),
        "url": event.url?.absoluteString ?? NSNull(),
        "has_recurrence": !(event.recurrenceRules?.isEmpty ?? true),
        "recurrence": recurrenceObject(event)
    ]
    if includeNotes { value["notes"] = event.notes ?? NSNull() }
    return value
}

private func parsePositiveInt(_ value: String?, default defaultValue: Int, name: String) throws -> Int {
    guard let value else { return defaultValue }
    guard let number = Int(value), number > 0 else {
        throw CalendarAdapterError.usage("\(name) must be a positive integer")
    }
    return number
}

private func parseURL(_ value: String) throws -> URL {
    guard let url = URL(string: value), url.scheme != nil else {
        throw CalendarAdapterError.usage("invalid --url; expected an absolute URL")
    }
    return url
}

private func recurrenceRule(args: inout Arguments) throws -> EKRecurrenceRule? {
    guard let repeatValue = try args.option("--repeat") else { return nil }
    let interval = try parsePositiveInt(try args.option("--interval"), default: 1, name: "--interval")
    let untilValue = try args.option("--until")
    let countValue = try args.option("--count")
    guard untilValue == nil || countValue == nil else {
        throw CalendarAdapterError.usage("use only one of --until or --count")
    }

    let end: EKRecurrenceEnd?
    if let untilValue {
        end = EKRecurrenceEnd(end: try Dates.day(untilValue, endOfDay: true))
    } else if let countValue {
        end = EKRecurrenceEnd(occurrenceCount: try parsePositiveInt(countValue, default: 1, name: "--count"))
    } else {
        end = nil
    }

    let frequency: EKRecurrenceFrequency
    switch repeatValue.lowercased() {
    case "daily": frequency = .daily
    case "weekly": frequency = .weekly
    case "monthly": frequency = .monthly
    case "yearly": frequency = .yearly
    default:
        throw CalendarAdapterError.usage("--repeat must be daily, weekly, monthly, or yearly")
    }
    return EKRecurrenceRule(recurrenceWith: frequency, interval: interval, end: end)
}

private func replaceRecurrenceEnd(
    on event: EKEvent,
    untilValue: String?,
    countValue: String?
) throws {
    guard untilValue != nil || countValue != nil else { return }
    guard untilValue == nil || countValue == nil else {
        throw CalendarAdapterError.usage("use only one of --until or --count")
    }

    let rules = event.recurrenceRules ?? []
    guard rules.count == 1, let rule = rules.first else {
        if rules.isEmpty {
            throw CalendarAdapterError.usage("--until and --count require a recurring event")
        }
        throw CalendarAdapterError.operation("events with multiple recurrence rules cannot be edited safely")
    }

    let end: EKRecurrenceEnd
    if let untilValue {
        let endDate = try Dates.day(untilValue, endOfDay: true)
        guard endDate >= event.startDate else {
            throw CalendarAdapterError.usage("--until must be the same as or after the selected event date")
        }
        end = EKRecurrenceEnd(end: endDate)
    } else {
        end = EKRecurrenceEnd(
            occurrenceCount: try parsePositiveInt(countValue, default: 1, name: "--count")
        )
    }

    let replacement = EKRecurrenceRule(
        recurrenceWith: rule.frequency,
        interval: rule.interval,
        daysOfTheWeek: rule.daysOfTheWeek,
        daysOfTheMonth: rule.daysOfTheMonth,
        monthsOfTheYear: rule.monthsOfTheYear,
        weeksOfTheYear: rule.weeksOfTheYear,
        daysOfTheYear: rule.daysOfTheYear,
        setPositions: rule.setPositions,
        end: end
    )
    event.removeRecurrenceRule(rule)
    event.addRecurrenceRule(replacement)
}

private func span(for event: EKEvent, value: String?) throws -> EKSpan {
    let recurring = !(event.recurrenceRules?.isEmpty ?? true)
    if recurring && value == nil {
        throw CalendarAdapterError.usage("recurring event requires --span this or --span future")
    }
    switch value?.lowercased() ?? "this" {
    case "this": return .thisEvent
    case "future": return .futureEvents
    default: throw CalendarAdapterError.usage("--span must be this or future")
    }
}

private let usage = """
sherpa planner calendar \(calendarAdapterVersion) — Apple Calendar EventKit CLI

Usage:
  sherpa planner calendar doctor [--json]
  sherpa planner calendar authorize [--json]
  sherpa planner calendar sources [--json]
  sherpa planner calendar calendars [--source NAME_OR_ID] [--json]
  sherpa planner calendar calendar-create NAME --source NAME_OR_ID [--color #RRGGBB] [--dry-run] [--json]
  sherpa planner calendar calendar-edit NAME_OR_ID [--source NAME_OR_ID] [--title TEXT]
      [--color #RRGGBB] [--dry-run] [--json]
  sherpa planner calendar calendar-delete NAME_OR_ID [--source NAME_OR_ID] --force [--json]
  sherpa planner calendar add TITLE --calendar NAME_OR_ID [--source NAME_OR_ID]
      (--date YYYY-MM-DD | --start DATETIME) [--end DATE_OR_DATETIME]
      [--notes TEXT] [--location TEXT] [--url URL]
      [--repeat daily|weekly|monthly|yearly] [--interval N]
      [--until YYYY-MM-DD | --count N] [--dry-run] [--json]
  sherpa planner calendar events [--calendar NAME_OR_ID] [--source NAME_OR_ID]
      [--from YYYY-MM-DD] [--to YYYY-MM-DD] [--query TEXT] [--limit N] [--json]
  sherpa planner calendar show EVENT_ID [--json]
  sherpa planner calendar edit EVENT_ID [--title TEXT] [--notes TEXT | --clear-notes]
      [--url URL | --clear-url]
      [--calendar NAME_OR_ID] [--source NAME_OR_ID] [--span this|future]
      [--until YYYY-MM-DD | --count N]
      [--dry-run] [--json]
  sherpa planner calendar move EVENT_ID --calendar NAME_OR_ID [--source NAME_OR_ID]
      [--span this|future] [--dry-run] [--json]
  sherpa planner calendar delete EVENT_ID [--span this|future] --force [--json]
  sherpa planner calendar --version
"""

@main
private struct CalendarAdapter {
    static func main() async {
        var raw = Array(CommandLine.arguments.dropFirst())
        if raw.isEmpty || raw.contains("--help") || raw.contains("-h") {
            print(usage)
            return
        }
        if raw == ["--version"] {
            print(calendarAdapterVersion)
            return
        }

        let command = raw.removeFirst()
        var args = Arguments(raw)
        let json = args.flag("--json")
        let output = Output(json: json)
        let calendarStore = CalendarStore()
        Logger.write(.info, "[cli:\(command):start] Command started")

        do {
            switch command {
            case "doctor":
                try args.finish()
                try output.object([
                    "version": calendarAdapterVersion,
                    "authorization": calendarStore.authorizationName(),
                    "calendar_count": calendarStore.authorizationName() == "full-access" ? calendarStore.calendars().count : 0
                ])

            case "authorize":
                try args.finish()
                let granted = try await calendarStore.authorize()
                try output.object(["granted": granted, "authorization": calendarStore.authorizationName()])

            case "sources":
                try args.finish()
                try calendarStore.requireFullAccess()
                try output.objects(calendarStore.sources().map(sourceObject))

            case "calendars":
                let sourceSelector = try args.option("--source")
                try args.finish()
                try calendarStore.requireFullAccess()
                var calendars = calendarStore.calendars()
                if let sourceSelector {
                    let source = try calendarStore.source(sourceSelector)
                    calendars = calendars.filter { $0.source.sourceIdentifier == source.sourceIdentifier }
                }
                try output.objects(calendars.map(calendarObject))

            case "calendar-create":
                try calendarStore.requireFullAccess()
                let title = try args.positional("calendar name")
                guard let sourceSelector = try args.option("--source") else {
                    throw CalendarAdapterError.usage("calendar-create requires --source to prevent creating in the wrong account")
                }
                let colorValue = try args.option("--color")
                let dryRun = args.flag("--dry-run")
                try args.finish()
                let source = try calendarStore.source(sourceSelector)
                let duplicate = calendarStore.calendars().contains {
                    $0.source.sourceIdentifier == source.sourceIdentifier && $0.title.caseInsensitiveCompare(title) == .orderedSame
                }
                guard !duplicate else {
                    throw CalendarAdapterError.operation("calendar '\(title)' already exists in source '\(source.title)'")
                }
                let calendar = EKCalendar(for: .event, eventStore: calendarStore.store)
                calendar.title = title
                calendar.source = source
                if let colorValue { calendar.cgColor = try parseColor(colorValue) }
                if !dryRun {
                    Logger.write(.info, "[eventkit:calendar-create:start] Saving calendar")
                    try calendarStore.store.saveCalendar(calendar, commit: true)
                    Logger.write(.info, "[eventkit:calendar-create:success] Calendar saved")
                }
                var value = calendarObject(calendar)
                value["dry_run"] = dryRun
                try output.object(value)

            case "calendar-edit":
                try calendarStore.requireFullAccess()
                let selector = try args.positional("calendar name or ID")
                let sourceSelector = try args.option("--source")
                let title = try args.option("--title")
                let colorValue = try args.option("--color")
                let dryRun = args.flag("--dry-run")
                try args.finish()
                guard title != nil || colorValue != nil else {
                    throw CalendarAdapterError.usage("calendar-edit requires --title or --color")
                }
                let calendar = try calendarStore.calendar(selector, sourceSelector: sourceSelector, writable: true)
                if let title {
                    let duplicate = calendarStore.calendars().contains {
                        $0.calendarIdentifier != calendar.calendarIdentifier &&
                        $0.source.sourceIdentifier == calendar.source.sourceIdentifier &&
                        $0.title.caseInsensitiveCompare(title) == .orderedSame
                    }
                    guard !duplicate else {
                        throw CalendarAdapterError.operation("calendar '\(title)' already exists in source '\(calendar.source.title)'")
                    }
                    calendar.title = title
                }
                if let colorValue { calendar.cgColor = try parseColor(colorValue) }
                if !dryRun {
                    Logger.write(.info, "[eventkit:calendar-edit:start] Saving calendar changes")
                    try calendarStore.store.saveCalendar(calendar, commit: true)
                    Logger.write(.info, "[eventkit:calendar-edit:success] Calendar changes saved")
                }
                var value = calendarObject(calendar)
                value["dry_run"] = dryRun
                try output.object(value)

            case "calendar-delete":
                try calendarStore.requireFullAccess()
                let selector = try args.positional("calendar name or ID")
                let sourceSelector = try args.option("--source")
                let force = args.flag("--force")
                try args.finish()
                guard force else {
                    throw CalendarAdapterError.usage("calendar-delete requires --force after reviewing the calendar and its events")
                }
                let calendar = try calendarStore.calendar(selector, sourceSelector: sourceSelector, writable: true)
                let preview = calendarObject(calendar)
                Logger.write(.warn, "[eventkit:calendar-delete:start] Deleting confirmed calendar")
                try calendarStore.store.removeCalendar(calendar, commit: true)
                Logger.write(.warn, "[eventkit:calendar-delete:success] Calendar deleted")
                try output.object(["deleted": true, "calendar": preview])

            case "add":
                try calendarStore.requireFullAccess()
                let title = try args.positional("event title")
                guard let calendarSelector = try args.option("--calendar") else {
                    throw CalendarAdapterError.usage("add requires --calendar")
                }
                let sourceSelector = try args.option("--source")
                let dayValue = try args.option("--date")
                let startValue = try args.option("--start")
                guard (dayValue != nil) != (startValue != nil) else {
                    throw CalendarAdapterError.usage("use exactly one of --date or --start")
                }
                let endValue = try args.option("--end")
                let notes = try args.option("--notes")
                let location = try args.option("--location")
                let urlValue = try args.option("--url")
                let rule = try recurrenceRule(args: &args)
                let dryRun = args.flag("--dry-run")
                try args.finish()

                let target = try calendarStore.calendar(calendarSelector, sourceSelector: sourceSelector, writable: true)
                let event = EKEvent(eventStore: calendarStore.store)
                event.title = title
                event.calendar = target
                event.notes = notes
                event.location = location
                if let urlValue {
                    event.url = try parseURL(urlValue)
                }
                if let dayValue {
                    event.isAllDay = true
                    event.startDate = try Dates.day(dayValue)
                    if let endValue {
                        event.endDate = try Dates.day(endValue)
                    } else {
                        event.endDate = event.startDate
                    }
                    if event.endDate < event.startDate { throw CalendarAdapterError.usage("--end must be the same as or after --date") }
                } else if let startValue {
                    event.isAllDay = false
                    event.startDate = try Dates.dateTime(startValue)
                    event.endDate = try endValue.map(Dates.dateTime) ?? event.startDate.addingTimeInterval(3600)
                    if event.endDate <= event.startDate { throw CalendarAdapterError.usage("--end must be after --start") }
                }
                if let rule { event.addRecurrenceRule(rule) }
                if !dryRun {
                    Logger.write(.info, "[eventkit:event-add:start] Saving event")
                    try calendarStore.store.save(event, span: .thisEvent)
                    Logger.write(.info, "[eventkit:event-add:success] Event saved")
                }
                var value = eventObject(event, includeNotes: true)
                value["dry_run"] = dryRun
                try output.object(value)

            case "events":
                try calendarStore.requireFullAccess()
                let calendarSelector = try args.option("--calendar")
                let sourceSelector = try args.option("--source")
                let fromValue = try args.option("--from")
                let toValue = try args.option("--to")
                let query = try args.option("--query")
                let limit = try parsePositiveInt(try args.option("--limit"), default: 200, name: "--limit")
                try args.finish()
                let start = try fromValue.map { try Dates.day($0) } ?? Calendar.current.startOfDay(for: Date())
                let end = try toValue.map { try Dates.day($0) } ?? Calendar.current.date(byAdding: .day, value: 30, to: start)!
                guard end > start else { throw CalendarAdapterError.usage("--to must be after --from") }
                let selectedCalendars: [EKCalendar]?
                if let calendarSelector {
                    selectedCalendars = [try calendarStore.calendar(calendarSelector, sourceSelector: sourceSelector)]
                } else if let sourceSelector {
                    let source = try calendarStore.source(sourceSelector)
                    selectedCalendars = calendarStore.calendars().filter { $0.source.sourceIdentifier == source.sourceIdentifier }
                } else {
                    selectedCalendars = nil
                }
                Logger.write(.info, "[eventkit:events:start] Fetching events")
                let predicate = calendarStore.store.predicateForEvents(withStart: start, end: end, calendars: selectedCalendars)
                var events = calendarStore.store.events(matching: predicate)
                if let query {
                    events = events.filter {
                        ($0.title ?? "").localizedCaseInsensitiveContains(query) ||
                        ($0.notes ?? "").localizedCaseInsensitiveContains(query)
                    }
                }
                events.sort { $0.startDate < $1.startDate }
                Logger.write(.info, "[eventkit:events:success] Events fetched")
                try output.objects(events.prefix(limit).map { eventObject($0) })

            case "show":
                try calendarStore.requireFullAccess()
                let identifier = try args.positional("event ID")
                try args.finish()
                try output.object(eventObject(try calendarStore.event(identifier), includeNotes: true))

            case "edit":
                try calendarStore.requireFullAccess()
                let identifier = try args.positional("event ID")
                let title = try args.option("--title")
                let notes = try args.option("--notes")
                let clearNotes = args.flag("--clear-notes")
                let urlValue = try args.option("--url")
                let clearURL = args.flag("--clear-url")
                let calendarSelector = try args.option("--calendar")
                let sourceSelector = try args.option("--source")
                let spanValue = try args.option("--span")
                let untilValue = try args.option("--until")
                let countValue = try args.option("--count")
                let dryRun = args.flag("--dry-run")
                try args.finish()
                guard title != nil || notes != nil || clearNotes || urlValue != nil || clearURL || calendarSelector != nil || untilValue != nil || countValue != nil else {
                    throw CalendarAdapterError.usage("edit requires --title, --notes, --clear-notes, --url, --clear-url, --calendar, --until, or --count")
                }
                guard !(notes != nil && clearNotes) else {
                    throw CalendarAdapterError.usage("use only one of --notes or --clear-notes")
                }
                guard !(urlValue != nil && clearURL) else {
                    throw CalendarAdapterError.usage("use only one of --url or --clear-url")
                }
                let event = try calendarStore.event(identifier)
                let selectedSpan = try span(for: event, value: spanValue)
                if let title { event.title = title }
                if let notes { event.notes = notes }
                if clearNotes { event.notes = nil }
                if let urlValue { event.url = try parseURL(urlValue) }
                if clearURL { event.url = nil }
                if let calendarSelector {
                    event.calendar = try calendarStore.calendar(calendarSelector, sourceSelector: sourceSelector, writable: true)
                }
                try replaceRecurrenceEnd(on: event, untilValue: untilValue, countValue: countValue)
                if !dryRun {
                    Logger.write(.info, "[eventkit:event-edit:start] Saving event changes")
                    try calendarStore.store.save(event, span: selectedSpan)
                    Logger.write(.info, "[eventkit:event-edit:success] Event changes saved")
                }
                var value = eventObject(event, includeNotes: true)
                value["dry_run"] = dryRun
                try output.object(value)

            case "move":
                try calendarStore.requireFullAccess()
                let identifier = try args.positional("event ID")
                guard let calendarSelector = try args.option("--calendar") else {
                    throw CalendarAdapterError.usage("move requires --calendar")
                }
                let sourceSelector = try args.option("--source")
                let spanValue = try args.option("--span")
                let dryRun = args.flag("--dry-run")
                try args.finish()
                let event = try calendarStore.event(identifier)
                let selectedSpan = try span(for: event, value: spanValue)
                event.calendar = try calendarStore.calendar(calendarSelector, sourceSelector: sourceSelector, writable: true)
                if !dryRun {
                    Logger.write(.info, "[eventkit:event-move:start] Moving event")
                    try calendarStore.store.save(event, span: selectedSpan)
                    Logger.write(.info, "[eventkit:event-move:success] Event moved")
                }
                var value = eventObject(event)
                value["dry_run"] = dryRun
                try output.object(value)

            case "delete":
                try calendarStore.requireFullAccess()
                let identifier = try args.positional("event ID")
                let spanValue = try args.option("--span")
                let force = args.flag("--force")
                try args.finish()
                guard force else { throw CalendarAdapterError.usage("delete requires --force after reviewing 'sherpa planner calendar show <event-id>'") }
                let event = try calendarStore.event(identifier)
                let selectedSpan = try span(for: event, value: spanValue)
                let preview = eventObject(event, includeNotes: true)
                Logger.write(.warn, "[eventkit:event-delete:start] Deleting confirmed event")
                try calendarStore.store.remove(event, span: selectedSpan)
                Logger.write(.warn, "[eventkit:event-delete:success] Event deleted")
                try output.object(["deleted": true, "event": preview])

            default:
                throw CalendarAdapterError.usage("unknown command '\(command)'\n\n\(usage)")
            }
            Logger.write(.info, "[cli:\(command):success] Command completed")
        } catch {
            Logger.write(.error, "[cli:\(command):failure] Command failed")
            FileHandle.standardError.write(Data("ERROR: \(error)\n".utf8))
            exit(1)
        }
    }
}
