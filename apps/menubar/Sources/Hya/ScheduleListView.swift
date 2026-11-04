import SwiftUI

/// Just the prayer rows: the header (location, refresh, mute) and the Quit footer live one
/// level up, in `HyaApp`'s own `content`, since they're shown no matter what `scheduleController`
/// has — this view only ever renders when there's a snapshot to show.
struct ScheduleListView: View {
    @ObservedObject var scheduleController: ScheduleController

    private static let timeFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .none
        formatter.timeStyle = .short
        return formatter
    }()

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            if let snapshot = scheduleController.snapshot {
                if let lastThird = snapshot.lastThird {
                    row("Last third", at: lastThird)
                }
                row("Fajr", snapshot.today.fajr)
                row("Sunrise", snapshot.today.sunrise)
                row("Dhuhr", snapshot.today.dhuhr)
                row("Asr", snapshot.today.asr)
                row("Maghrib", snapshot.today.maghrib)
                row("Isha", snapshot.today.isha)
                if let midnight = snapshot.midnight {
                    row("Midnight", at: midnight)
                }
            }
        }
    }

    private func row(_ name: String, _ window: Window) -> some View {
        HStack {
            Text(name)
            Spacer()
            Text(Self.timeFormatter.string(from: Date(timeIntervalSince1970: TimeInterval(window.at))))
                .monospacedDigit()
                .foregroundStyle(.secondary)
        }
        .fontWeight(isActive(window, at: Date()) ? .bold : .regular)
    }

    /// Midnight and the last third are reference marks, not prayer windows: no `ends`, so never
    /// bold like an active prayer.
    private func row(_ name: String, at timestamp: Int64) -> some View {
        HStack {
            Text(name)
            Spacer()
            Text(Self.timeFormatter.string(from: Date(timeIntervalSince1970: TimeInterval(timestamp))))
                .monospacedDigit()
                .foregroundStyle(.secondary)
        }
    }
}
