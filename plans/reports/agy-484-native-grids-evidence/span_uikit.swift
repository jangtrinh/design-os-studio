let gridFrame = UIStackView()
gridFrame.axis = .vertical
gridFrame.spacing = 12
gridFrame.alignment = .leading
gridFrame.isLayoutMarginsRelativeArrangement = true
gridFrame.layoutMargins = UIEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
gridFrame.translatesAutoresizingMaskIntoConstraints = false
NSLayoutConstraint.activate([
    gridFrame.widthAnchor.constraint(equalToConstant: 428),
    gridFrame.heightAnchor.constraint(equalToConstant: 220),
])
let row = UIStackView()
row.axis = .horizontal
row.spacing = 12
row.alignment = .top
gridFrame.addArrangedSubview(row)
// "HeroTile": row span 2 is not supported by UIKit stack grid; placed in row 1
let cell = UIView()
cell.translatesAutoresizingMaskIntoConstraints = false
NSLayoutConstraint.activate([
    cell.widthAnchor.constraint(equalToConstant: 196),
    cell.heightAnchor.constraint(equalToConstant: 40),
])
row.addArrangedSubview(cell)
let heroTile = UIView()
heroTile.backgroundColor = UIColor(red: 0.208, green: 0.51, blue: 1, alpha: 1)
heroTile.translatesAutoresizingMaskIntoConstraints = false
cell.addSubview(heroTile)
NSLayoutConstraint.activate([
    heroTile.widthAnchor.constraint(equalToConstant: 196),
    heroTile.heightAnchor.constraint(equalToConstant: 92),
    heroTile.leadingAnchor.constraint(equalTo: cell.leadingAnchor),
    heroTile.topAnchor.constraint(equalTo: cell.topAnchor),
])
let cell2 = UIView()
cell2.translatesAutoresizingMaskIntoConstraints = false
NSLayoutConstraint.activate([
    cell2.widthAnchor.constraint(equalToConstant: 196),
    cell2.heightAnchor.constraint(equalToConstant: 40),
])
row.addArrangedSubview(cell2)
let topWide = UIView()
topWide.backgroundColor = UIColor(red: 0.208, green: 0.51, blue: 1, alpha: 1)
topWide.translatesAutoresizingMaskIntoConstraints = false
cell2.addSubview(topWide)
NSLayoutConstraint.activate([
    topWide.widthAnchor.constraint(equalToConstant: 196),
    topWide.heightAnchor.constraint(equalToConstant: 40),
    topWide.leadingAnchor.constraint(equalTo: cell2.leadingAnchor),
    topWide.topAnchor.constraint(equalTo: cell2.topAnchor),
])
let row2 = UIStackView()
row2.axis = .horizontal
row2.spacing = 12
row2.alignment = .top
gridFrame.addArrangedSubview(row2)
let hole = UIView()
hole.translatesAutoresizingMaskIntoConstraints = false
NSLayoutConstraint.activate([
    hole.widthAnchor.constraint(equalToConstant: 92),
    hole.heightAnchor.constraint(equalToConstant: 40),
])
row2.addArrangedSubview(hole)
let hole2 = UIView()
hole2.translatesAutoresizingMaskIntoConstraints = false
NSLayoutConstraint.activate([
    hole2.widthAnchor.constraint(equalToConstant: 92),
    hole2.heightAnchor.constraint(equalToConstant: 40),
])
row2.addArrangedSubview(hole2)
let cell3 = UIView()
cell3.translatesAutoresizingMaskIntoConstraints = false
NSLayoutConstraint.activate([
    cell3.widthAnchor.constraint(equalToConstant: 92),
    cell3.heightAnchor.constraint(equalToConstant: 40),
])
row2.addArrangedSubview(cell3)
let midA = UIView()
midA.backgroundColor = UIColor(red: 0.208, green: 0.51, blue: 1, alpha: 1)
midA.translatesAutoresizingMaskIntoConstraints = false
cell3.addSubview(midA)
NSLayoutConstraint.activate([
    midA.widthAnchor.constraint(equalToConstant: 92),
    midA.heightAnchor.constraint(equalToConstant: 40),
    midA.leadingAnchor.constraint(equalTo: cell3.leadingAnchor),
    midA.topAnchor.constraint(equalTo: cell3.topAnchor),
])
let cell4 = UIView()
cell4.translatesAutoresizingMaskIntoConstraints = false
NSLayoutConstraint.activate([
    cell4.widthAnchor.constraint(equalToConstant: 92),
    cell4.heightAnchor.constraint(equalToConstant: 40),
])
row2.addArrangedSubview(cell4)
let midB = UIView()
midB.backgroundColor = UIColor(red: 0.208, green: 0.51, blue: 1, alpha: 1)
midB.translatesAutoresizingMaskIntoConstraints = false
cell4.addSubview(midB)
NSLayoutConstraint.activate([
    midB.widthAnchor.constraint(equalToConstant: 92),
    midB.heightAnchor.constraint(equalToConstant: 40),
    midB.leadingAnchor.constraint(equalTo: cell4.leadingAnchor),
    midB.topAnchor.constraint(equalTo: cell4.topAnchor),
])
let row3 = UIStackView()
row3.axis = .horizontal
row3.spacing = 12
row3.alignment = .top
gridFrame.addArrangedSubview(row3)
let cell5 = UIView()
cell5.translatesAutoresizingMaskIntoConstraints = false
NSLayoutConstraint.activate([
    cell5.widthAnchor.constraint(equalToConstant: 404),
    cell5.heightAnchor.constraint(equalToConstant: 36),
])
row3.addArrangedSubview(cell5)
let footerBar = UIView()
footerBar.backgroundColor = UIColor(red: 0.208, green: 0.51, blue: 1, alpha: 1)
footerBar.translatesAutoresizingMaskIntoConstraints = false
cell5.addSubview(footerBar)
NSLayoutConstraint.activate([
    footerBar.widthAnchor.constraint(equalToConstant: 404),
    footerBar.heightAnchor.constraint(equalToConstant: 36),
    footerBar.leadingAnchor.constraint(equalTo: cell5.leadingAnchor),
    footerBar.topAnchor.constraint(equalTo: cell5.topAnchor),
])