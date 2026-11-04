import SwiftUI

struct LocationHeaderView: View {
    @State private var latitude: String
    @State private var longitude: String
    @State private var muteBeforePrayerEnabled: Bool

    let onApply: (Coordinates) -> Void

    init(onApply: @escaping (Coordinates) -> Void) {
        self.onApply = onApply
        let initial = Self.initialText(for: SettingsStore.shared.coordinates)
        _latitude = State(initialValue: initial.latitude)
        _longitude = State(initialValue: initial.longitude)
        _muteBeforePrayerEnabled = State(initialValue: SettingsStore.shared.muteBeforePrayerEnabled)
    }

    static func initialText(for coordinates: Coordinates?) -> (latitude: String, longitude: String) {
        guard let coordinates else { return ("", "") }
        return ("\(coordinates.latitude)", "\(coordinates.longitude)")
    }

    private var validated: Coordinates? {
        try? Coordinates.validated(latitude: latitude, longitude: longitude).get()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .center, spacing: 8) {
                Grid(alignment: .leading, horizontalSpacing: 8, verticalSpacing: 4) {
                    GridRow {
                        Text("Latitude")
                        TextField("", text: $latitude).compactField()
                    }
                    GridRow {
                        Text("Longitude")
                        TextField("", text: $longitude).compactField()
                    }
                }
                Button {
                    guard let coordinates = validated else { return }
                    onApply(coordinates)
                } label: {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.bordered)
                .disabled(validated == nil)
                .help("Apply and refresh now")
            }
            Toggle("Mute audio 5 minutes before each prayer", isOn: $muteBeforePrayerEnabled)
                .onChange(of: muteBeforePrayerEnabled) { enabled in
                    SettingsStore.shared.setMuteBeforePrayerEnabled(enabled)
                    if enabled {
                        requestNotificationAuthorization()
                    }
                }
        }
    }
}

private extension View {
    func compactField() -> some View {
        textFieldStyle(.plain)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(
                RoundedRectangle(cornerRadius: 5, style: .continuous)
                    .fill(.quaternary)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 5, style: .continuous)
                    .strokeBorder(.separator, lineWidth: 0.5)
            )
    }
}
