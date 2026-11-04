import AppKit
import SwiftUI

struct ScheduleListView: View {
    @ObservedObject var scheduleController: ScheduleController
    let onSettings: () -> Void

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
            Divider()
            footer
        }
        .padding()
        .frame(minWidth: 220)
    }

    private var footer: some View {
        HStack {
            Button("Settings…", action: onSettings)
                .buttonStyle(.borderless)
            Spacer()
            Button("Quit") { NSApplication.shared.terminate(nil) }
                .buttonStyle(.borderless)
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
