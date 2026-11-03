import SwiftUI

struct SettingsView: View {
    @State private var latitude: String
    @State private var longitude: String

    let onSave: () -> Void
    let onCancel: (() -> Void)?

    init(onSave: @escaping () -> Void, onCancel: (() -> Void)?) {
        self.onSave = onSave
        self.onCancel = onCancel
        let initial = Self.initialText(for: SettingsStore.shared.coordinates)
        _latitude = State(initialValue: initial.latitude)
        _longitude = State(initialValue: initial.longitude)
    }

    /// The fields' starting text: the existing saved coordinates when reopening Settings to
    /// review or change them, or blank on a true first run.
    static func initialText(for coordinates: Coordinates?) -> (latitude: String, longitude: String) {
        guard let coordinates else { return ("", "") }
        return ("\(coordinates.latitude)", "\(coordinates.longitude)")
    }

    private var validated: Coordinates? {
        try? Coordinates.validated(latitude: latitude, longitude: longitude).get()
    }

    var body: some View {
        Form {
            TextField("Latitude", text: $latitude)
            TextField("Longitude", text: $longitude)
            HStack {
                if let onCancel {
                    Button("Cancel", action: onCancel)
                }
                Spacer()
                Button("Save") {
                    guard let coordinates = validated else { return }
                    SettingsStore.shared.save(coordinates)
                    onSave()
                }
                .disabled(validated == nil)
            }
        }
        .padding()
    }
}
