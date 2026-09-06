// Shadow — applicazione della barra dei menu di macOS.
//
// Il segno del prodotto sono **tre barre orizzontali**. Nella barra dei menu è
// disegnato (`statusIcon`), nei testi è il carattere `≡`: uno solo dei due
// posti userebbe un marchio diverso, e un marchio diverso nello stesso schermo
// fa sembrare l'applicazione rotta.
//
// # Cosa fa, e cosa deliberatamente non fa
//
// Mostra la **stessa vista** del terminale, della pagina e del server: la
// legge chiamando `shadow status --format json`. Non parla con un database,
// non classifica niente, non apre nessuna porta. Se calcolasse qualcosa per
// conto suo, prima o poi direbbe un numero diverso dal report — ed è
// esattamente ciò che il progetto evita tenendo una vista sola.
//
// La finestra estesa mostra la pagina prodotta da `shadow dashboard`: la stessa
// che si apre col doppio clic e la stessa che il server servirebbe. Un solo
// documento, quattro modi di guardarlo.

import AppKit
import SwiftUI
import WebKit

// MARK: - Le impostazioni

/// Dove l'app trova ciò che le serve.
///
/// Un file JSON invece di un pannello di preferenze: è ispezionabile, si mette
/// sotto controllo di versione, e si configura da terminale come tutto il resto
/// di questo strumento.
struct Settings: Codable {
    var shadowPath: String
    var historyPath: String
    var target: String
    var refreshSeconds: Int

    static let directory = FileManager.default
        .homeDirectoryForCurrentUser
        .appendingPathComponent("Library/Application Support/Shadow", isDirectory: true)
    static let file = directory.appendingPathComponent("menubar.json")

    static func load() -> Settings? {
        guard let data = try? Data(contentsOf: file) else { return nil }
        return try? JSONDecoder().decode(Settings.self, from: data)
    }

    /// Scrive un file di esempio la prima volta, invece di lasciare l'utente
    /// davanti a una finestra vuota senza sapere cosa manchi.
    static func writeTemplate() {
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let template = Settings(
            shadowPath: "/usr/local/bin/shadow",
            historyPath: NSString(string: "~/shadow/history.db").expandingTildeInPath,
            target: "default",
            refreshSeconds: 30
        )
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try? encoder.encode(template).write(to: file)
    }
}

// MARK: - Il modello, letto dal binario

struct Endpoint: Decodable, Identifiable {
    let id: String
    let path_pattern: String
    let classification: String?
    let first_seen_run: Int?
}

struct LastRun: Decodable {
    let id: Int
    let timestamp: String
    let endpoints: Int
    let lines: Int
}

struct Status: Decodable {
    let target: String
    let last_run: LastRun?
    let shadow_version: String
    let ruleset_version: String
    let new_in_last_run: [Endpoint]
    let open_alerts: [Endpoint]
    let inventory_size: Int
    let by_classification: [String: Int]
}

/// Che cosa è andato storto, detto in modo che si possa rimediare.
enum LoadError: LocalizedError {
    case noSettings
    case cannotRun(String)
    case toolFailed(String)
    case badOutput

    var errorDescription: String? {
        switch self {
        case .noSettings:
            return "Set up \(Settings.file.path) with the path to the shadow binary, "
                + "the history file and the target, then choose Refresh."
        case .cannotRun(let path):
            return "Cannot run \(path). Check the shadowPath in the settings file."
        case .toolFailed(let message):
            return message.isEmpty ? "shadow status failed." : message
        case .badOutput:
            return "shadow status returned something this app could not read."
        }
    }
}

/// Legge la vista chiamando il binario.
///
/// Nessun argomento arriva dall'esterno: percorsi e bersaglio vengono dal file
/// di impostazioni dell'utente, non da un input di rete. È la ragione per cui
/// questa app non ha una superficie di attacco propria.
enum Reader {
    static func status(_ settings: Settings) throws -> Status {
        let data = try run(settings, arguments: [
            "status", "--history", settings.historyPath, "--target", settings.target,
            "--format", "json",
        ])
        guard let status = try? JSONDecoder().decode(Status.self, from: data) else {
            throw LoadError.badOutput
        }
        return status
    }

    static func dashboardHTML(_ settings: Settings) throws -> String {
        let data = try run(settings, arguments: [
            "dashboard", "--history", settings.historyPath, "--target", settings.target,
        ])
        guard let html = String(data: data, encoding: .utf8) else { throw LoadError.badOutput }
        return html
    }

    private static func run(_ settings: Settings, arguments: [String]) throws -> Data {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: settings.shadowPath)
        process.arguments = arguments
        let out = Pipe()
        let err = Pipe()
        process.standardOutput = out
        process.standardError = err
        do {
            try process.run()
        } catch {
            throw LoadError.cannotRun(settings.shadowPath)
        }
        let data = out.fileHandleForReading.readDataToEndOfFile()
        let errorText = String(
            data: err.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw LoadError.toolFailed(errorText.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        return data
    }
}

// MARK: - Lo stato dell'app

@MainActor
final class Model: ObservableObject {
    @Published var status: Status?
    @Published var failure: String?
    @Published var lastRefresh: Date?

    private var timer: Timer?

    func start() {
        refresh()
        let seconds = Double(Settings.load()?.refreshSeconds ?? 30)
        // `self` si lega a una costante **prima** del `Task`: catturarlo dentro
        // un contesto concorrente è un errore su alcune versioni del
        // compilatore e un'ambiguità su tutte le altre.
        timer = Timer.scheduledTimer(withTimeInterval: max(seconds, 5), repeats: true) { [weak self] _ in
            guard let model = self else { return }
            Task { @MainActor in model.refresh() }
        }
    }

    func refresh() {
        guard let settings = Settings.load() else {
            Settings.writeTemplate()
            failure = LoadError.noSettings.errorDescription
            status = nil
            return
        }
        do {
            status = try Reader.status(settings)
            failure = nil
            lastRefresh = Date()
        } catch {
            failure = error.localizedDescription
            status = nil
        }
    }

    /// Quanti alert aspettano una decisione. È il solo numero che finisce nella
    /// barra dei menu: se ce ne fossero due, nessuno dei due si leggerebbe.
    var openAlerts: Int { status?.open_alerts.count ?? 0 }
}

// MARK: - Il popover

struct PopoverView: View {
    @ObservedObject var model: Model
    var onExpand: () -> Void
    var onQuit: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            Divider()
            if let failure = model.failure {
                Text(failure)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            } else if let status = model.status {
                changed(status)
                alerts(status)
                inventory(status)
            } else {
                Text("Reading…").foregroundStyle(.secondary)
            }
            Divider()
            footer
        }
        .padding(14)
        .frame(width: 330)
    }

    private var header: some View {
        HStack(alignment: .firstTextBaseline) {
            Text("\u{2261}").font(.system(.title3, design: .monospaced)).foregroundStyle(.tint)
            Text(model.status?.target ?? "Shadow").font(.headline)
            Spacer()
            if let run = model.status?.last_run {
                Text("run \(run.id)").font(.caption).foregroundStyle(.secondary)
            }
        }
    }

    private func changed(_ status: Status) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("WHAT CHANGED").font(.caption2).foregroundStyle(.secondary)
            if status.new_in_last_run.isEmpty {
                Text("Nothing new in the last run.").font(.callout).foregroundStyle(.secondary)
            } else {
                ForEach(status.new_in_last_run.prefix(4)) { endpoint in
                    Text(endpoint.path_pattern)
                        .font(.system(.caption, design: .monospaced))
                        .lineLimit(1).truncationMode(.middle)
                }
                if status.new_in_last_run.count > 4 {
                    Text("and \(status.new_in_last_run.count - 4) more")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }
        }
    }

    private func alerts(_ status: Status) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("OPEN ALERTS").font(.caption2).foregroundStyle(.secondary)
            if status.open_alerts.isEmpty {
                Text("None waiting.").font(.callout).foregroundStyle(.secondary)
            } else {
                ForEach(status.open_alerts.prefix(4)) { endpoint in
                    Text(endpoint.path_pattern)
                        .font(.system(.caption, design: .monospaced))
                        .lineLimit(1).truncationMode(.middle)
                }
                if status.open_alerts.count > 4 {
                    Text("and \(status.open_alerts.count - 4) more")
                        .font(.caption).foregroundStyle(.secondary)
                }
            }
        }
    }

    private func inventory(_ status: Status) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("INVENTORY — \(status.inventory_size) ENDPOINT(S)")
                .font(.caption2).foregroundStyle(.secondary)
            HStack(spacing: 10) {
                // Ordine fisso, **zeri compresi**: una categoria che sparisce
                // quando è vuota fa credere che non esista.
                ForEach(["Shadow", "Zombie", "Known", "Undetermined"], id: \.self) { name in
                    VStack(spacing: 1) {
                        Text("\(status.by_classification[name] ?? 0)")
                            .font(.system(.callout, design: .rounded)).bold()
                        Text(name).font(.caption2).foregroundStyle(.secondary)
                    }
                }
            }
        }
    }

    private var footer: some View {
        HStack {
            Button("Open full view", action: onExpand)
            Spacer()
            Button("Refresh") { model.refresh() }
            Button("Quit", action: onQuit)
        }
        .buttonStyle(.link)
        .font(.caption)
    }
}

// MARK: - Il pannello dei dettagli

/// I dettagli, in una finestra posizionata **a mano** sotto la barra dei menu.
///
/// # Perché non un `NSPopover`
///
/// Un popover disegna una freccia e si tiene il proprio margine, e quel margine
/// non si controlla: il pannello finiva staccato dalla barra. Qui la posizione
/// la calcoliamo noi — il bordo superiore del pannello coincide con il bordo
/// inferiore della barra dei menu — e il risultato è quello che si vede in ogni
/// applicazione che vive lassù: **adiacente**, senza stacco e senza freccia.
@MainActor
final class DetailPanel {
    private let panel: NSPanel
    private let hosting: NSHostingView<PopoverView>
    private let background: NSVisualEffectView
    private var outsideClick: Any?

    init(rootView: PopoverView) {
        hosting = NSHostingView(rootView: rootView)
        // Lo sfondo lo dava il popover; una finestra senza bordo non ne ha
        // nessuno, e senza questo il pannello sarebbe trasparente. Il materiale
        // è quello che macOS usa per i popover, così il pannello somiglia a ciò
        // che si aspetta chi lo apre dalla barra dei menu.
        background = NSVisualEffectView()
        background.material = .popover
        background.blendingMode = .behindWindow
        background.state = .active
        panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 330, height: 320),
            // Senza bordo e **non attivante**: aprirlo non deve rubare il fuoco
            // all'applicazione con cui si stava lavorando.
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false)
        background.addSubview(hosting)
        hosting.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            hosting.topAnchor.constraint(equalTo: background.topAnchor),
            hosting.bottomAnchor.constraint(equalTo: background.bottomAnchor),
            hosting.leadingAnchor.constraint(equalTo: background.leadingAnchor),
            hosting.trailingAnchor.constraint(equalTo: background.trailingAnchor),
        ])
        panel.contentView = background
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        // Sopra le finestre normali, come si aspetta chi apre qualcosa dalla
        // barra dei menu.
        panel.level = .statusBar
        panel.hidesOnDeactivate = false
        panel.isMovable = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

        // L'angolo arrotondato lo disegna il livello: la finestra non ha bordo,
        // quindi senza questo sarebbe un rettangolo netto. Va sullo **sfondo**,
        // non sul contenuto, o gli angoli resterebbero pieni.
        background.wantsLayer = true
        background.layer?.cornerRadius = 10
        background.layer?.masksToBounds = true
    }

    var isVisible: Bool { panel.isVisible }

    /// Mostra il pannello **attaccato** alla barra dei menu, sotto l'elemento.
    func show(under button: NSStatusBarButton) {
        guard let barWindow = button.window, let screen = barWindow.screen ?? NSScreen.main else {
            return
        }
        // L'altezza la decide il contenuto: una finestra alta quanto un valore
        // fisso mostrerebbe uno spazio vuoto quando non c'è niente da dire.
        let size = hosting.fittingSize
        let width = max(size.width, 330)
        let height = max(size.height, 120)

        // Le coordinate del **bottone**, non della sua finestra: la finestra di
        // un elemento della barra è più larga del segno che ci si vede dentro,
        // e centrarsi su quella spostava il pannello di una cinquantina di
        // punti a destra. Misurato guardando uno screenshot, non dedotto.
        let inWindow = button.convert(button.bounds, to: nil)
        let bar = barWindow.convertToScreen(inWindow)
        var x = bar.midX - width / 2
        // Se l'elemento è vicino al bordo, il pannello rientra invece di uscire
        // dallo schermo.
        let visible = screen.visibleFrame
        x = min(max(x, visible.minX + 8), visible.maxX - width - 8)

        // Il bordo superiore del pannello **è** il bordo inferiore della barra.
        let y = bar.minY - height

        panel.setFrame(NSRect(x: x, y: y, width: width, height: height), display: true)
        panel.orderFrontRegardless()
        watchForOutsideClick()
    }

    func close() {
        stopWatching()
        panel.orderOut(nil)
    }

    /// Chiude quando si clicca altrove, che è ciò che un popover faceva da sé.
    private func watchForOutsideClick() {
        stopWatching()
        outsideClick = NSEvent.addGlobalMonitorForEvents(
            matching: [.leftMouseDown, .rightMouseDown]
        ) { [weak self] _ in
            Task { @MainActor in self?.close() }
        }
    }

    private func stopWatching() {
        if let monitor = outsideClick {
            NSEvent.removeMonitor(monitor)
            outsideClick = nil
        }
    }
}

// MARK: - La finestra estesa

/// Mostra **la stessa pagina** che `shadow dashboard` scrive su file e che il
/// server servirebbe.
///
/// Il JavaScript è spento: la pagina non ne contiene, e una vista che non
/// esegue niente è una vista che non può essere sorpresa da un path costruito
/// ad arte finito nell'HTML.
final class DashboardWindow: NSWindowController {
    private let webView: WKWebView

    init() {
        let configuration = WKWebViewConfiguration()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        webView = WKWebView(frame: .zero, configuration: configuration)

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 900, height: 680),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false)
        window.title = "Shadow"
        window.contentView = webView
        window.center()
        super.init(window: window)
    }

    required init?(coder: NSCoder) { fatalError("non usato") }

    func show(html: String) {
        webView.loadHTMLString(html, baseURL: nil)
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func show(error: String) {
        let escaped = error
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
        show(html: """
            <!doctype html><meta charset="utf-8">
            <body style="font:14px -apple-system,sans-serif;padding:2rem;color:#555">
            <p>\(escaped)</p></body>
            """)
    }
}

// MARK: - L'icona della barra dei menu

/// Il segno: **tre barre orizzontali**, disegnate invece che scritte.
///
/// È un'immagine *template*: macOS la ricolora da sé secondo il tema della
/// barra dei menu, chiara o scura, e resta nitida su schermi a densità diversa
/// perché è vettoriale e non un file di pixel.
func statusIcon() -> NSImage {
    let size = NSSize(width: 18, height: 14)
    let image = NSImage(size: size, flipped: false) { rect in
        let path = NSBezierPath()
        path.lineWidth = 1.7
        // Estremità arrotondate: è la differenza fra tre barre e tre tagli.
        path.lineCapStyle = .round

        let inset: CGFloat = 1.5
        let spacing: CGFloat = 4.2
        let middle = rect.midY
        for offset in [spacing, 0, -spacing] {
            path.move(to: NSPoint(x: rect.minX + inset, y: middle + offset))
            path.line(to: NSPoint(x: rect.maxX - inset, y: middle + offset))
        }

        NSColor.black.setStroke()
        path.stroke()
        return true
    }
    image.isTemplate = true
    return image
}

// MARK: - L'applicazione

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem!
    private var details: DetailPanel!
    private let model = Model()
    private lazy var dashboard = DashboardWindow()

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem.button?.image = statusIcon()
        statusItem.button?.imagePosition = .imageLeading
        statusItem.button?.action = #selector(toggle)
        statusItem.button?.target = self

        details = DetailPanel(
            rootView: PopoverView(
                model: model,
                onExpand: { [weak self] in self?.expand() },
                onQuit: { NSApp.terminate(nil) }))

        model.start()
        observeAlerts()
    }

    /// Il numero accanto al segno **solo quando c'è qualcosa da decidere**: una
    /// barra dei menu che mostra sempre un numero smette di essere guardata.
    private func observeAlerts() {
        Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
            guard let delegate = self else { return }
            Task { @MainActor in
                let open = delegate.model.openAlerts
                delegate.statusItem.button?.title = open > 0 ? " \(open)" : ""
            }
        }
    }

    @objc private func toggle() {
        // Primo clic: i dettagli. Ancora: la finestra di visione.
        if details.isVisible {
            details.close()
            expand()
            return
        }
        model.refresh()
        if let button = statusItem.button {
            details.show(under: button)
        }
    }

    private func expand() {
        guard let settings = Settings.load() else {
            dashboard.show(error: LoadError.noSettings.errorDescription ?? "")
            return
        }
        do {
            dashboard.show(html: try Reader.dashboardHTML(settings))
        } catch {
            dashboard.show(error: error.localizedDescription)
        }
    }
}

/// L'avvio, esplicitamente sul thread principale.
///
/// Swift 6 non lo dà per scontato nel codice di primo livello, e ha ragione:
/// una barra dei menu toccata da un thread qualsiasi è un difetto che si
/// manifesta una volta su cento.
@MainActor
func start() -> Never {
    let app = NSApplication.shared
    let delegate = AppDelegate()
    app.delegate = delegate
    // Nessuna icona nel Dock: vive nella barra dei menu.
    app.setActivationPolicy(.accessory)
    // `delegate` è tenuto in vita da `app`, ma lo si dichiara qui perché sia
    // evidente che non è un temporaneo.
    withExtendedLifetime(delegate) { app.run() }
    exit(0)
}

MainActor.assumeIsolated { start() }
