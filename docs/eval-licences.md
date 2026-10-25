# Evaluation, Model-Weight, Design-Kit & Skill Licence Register

This document is the single source of truth for the licence, distribution class, attribution text, and review status of every external dataset, model weight, design kit, skill collection, knowledge source, and bundled font referenced by `designos-eval`, `skills/`, or the Studio application (`E15-23`, issue `#611`).

The machine-readable `toml` block below is validated in CI by `python3 scripts/check_eval_licences.py` and used to generate `THIRD_PARTY_LICENSES.md` (`python3 scripts/check_eval_licences.py --write`). Every row in the summary table must match a `[[source]]` entry in the `toml` block (`id`, `licence`, and `use` in the same order).

Rules the checker enforces on every entry:

- `pinned` is an immutable upstream revision: a 40-hex git commit or Hugging Face revision, or `gcs-<generation>` for an object in a Google Cloud Storage bucket. `pin_source` is the URL that proves it. A tag, a year or a version label is refused.
- `licence` is an SPDX id, `LicenseRef-<name>` for terms that have no SPDX id (the terms go verbatim into `notes`), or `NONE`.
- `review = "approved"` needs a `reviewer` and `reviewed_on`, and only records an owner ruling that exists (README-part2 owner rulings). Everything else stays `pending` with an empty `reviewer` and `reviewed_on`; `designos-eval` refuses to run a task whose source is not approved.
- A `ship-*` entry carries `licence_text_file`, the full licence text, which is embedded in `THIRD_PARTY_LICENSES.md`.

Pins were taken from the GitHub and Hugging Face APIs on 2026-10-19 (the upstream head at that time, except where a published revision is already named in the spec). Pending entries still carry their pin so that approval attaches to a known revision.

## Summary Table

| ID | Licence | Use | Name | Kind | Review | Consumers |
|---|---|---|---|---|---|---|
| `design2code` | `ODC-By-1.0` | `eval-only-cache` | Design2Code test set (`SALT-NLP/Design2Code`) | `dataset` | `approved` | `E15-27` |
| `design2code-code` | `MIT` | `eval-only-cache` | Design2Code metric reference (`NoviScl/Design2Code`) | `dataset` | `pending` | `E15-27` |
| `websight` | `CC-BY-4.0` | `eval-only-cache` | HuggingFaceM4 WebSight synthetic pages | `dataset` | `pending` | `E15-27` |
| `uicrit` | `CC-BY-4.0` | `eval-only-cache` | Google Research UICrit critiques and ratings | `dataset` | `approved` | `E15-32` |
| `rico-screenshots` | `LicenseRef-RICO-Terms` | `eval-only-cache` | RICO mobile UI screenshot corpus | `dataset` | `approved` | `E15-32` |
| `uiclip` | `MIT` | `eval-only-cache` | UIClip model weights (`biglab/uiclip_jitteredwebsites-2-224-paraphrased`) | `weights` | `approved` | `E15-31`, `E15-37` |
| `clip-vit-b32` | `MIT` | `eval-only-cache` | OpenAI CLIP ViT-B/32 weights (`openai/clip-vit-base-patch32`) | `weights` | `approved` | `E15-31`, `E15-37` |
| `shadcn-ui` | `MIT` | `eval-redistribute` | shadcn/ui open component and token templates | `kit` | `approved` | `E15-24`, `E15-28`, `E15-29`, `E15-30` |
| `ibm-carbon` | `Apache-2.0` | `eval-redistribute` | IBM Carbon Design System tokens and components | `kit` | `approved` | `E15-24`, `E15-28`, `E15-29`, `E15-30` |
| `jakubkrehel-skills` | `MIT` | `ship-in-skills` | jakubkrehel/skills UI craft rules | `skills` | `approved` | `E15-46`, `E15-47` |
| `transitions-dev` | `NONE` | `excluded` | transitions.dev skills (no licence published) | `skills` | `rejected` | — |
| `ease-design` | `MIT` | `ship-in-skills` | ease-design first-party knowledge | `knowledge` | `approved` | `E15-18`, `E15-21`, `E15-31`, `E15-47` |
| `font-inter` | `OFL-1.1` | `ship-in-app` | Inter typeface | `font` | `pending` | `E15-18` |
| `figma-client-files` | `NONE` | `excluded` | Proprietary client and user-home design files | `dataset` | `rejected` | — |

## Machine Register

```toml
[[source]]
id = "design2code"
name = "Design2Code test set (SALT-NLP/Design2Code)"
kind = "dataset"
url = "https://huggingface.co/datasets/SALT-NLP/Design2Code"
pinned = "e2f61f6e9d9b5ee201b81d8e0ae4d37aa13f8f25"
pin_source = "https://huggingface.co/api/datasets/SALT-NLP/Design2Code/revision/e2f61f6e9d9b5ee201b81d8e0ae4d37aa13f8f25"
licence = "ODC-By-1.0"
use = "eval-only-cache"
attribution = "Design2Code (Si et al., 2024), SALT-NLP, ODC-BY 1.0."
review = "approved"
reviewer = "owner"
reviewed_on = "2026-10-19"
notes = "Owner ruling 2026-10-19 (README-part2 owner answers): approved as eval-only-cache only; downloaded at run time into the user cache by designos-eval fetch, never committed or shipped. Pages derive from third-party websites."
consumers = ["E15-27"]

[[source]]
id = "design2code-code"
name = "Design2Code metric reference (NoviScl/Design2Code)"
kind = "dataset"
url = "https://github.com/NoviScl/Design2Code"
pinned = "7a575e4c33f417c4be5c64072b8f5798de0d0f99"
pin_source = "https://api.github.com/repos/NoviScl/Design2Code/commits/7a575e4c33f417c4be5c64072b8f5798de0d0f99"
licence = "MIT"
use = "eval-only-cache"
attribution = "Copyright (c) 2023 Chenglei Si (MIT License)."
review = "pending"
reviewer = ""
reviewed_on = ""
notes = "Reference metric implementation (CODE_LICENSE is MIT; its DATA_LICENSE is the ODC-By text of the dataset). The owner ruling names Design2Code, not this code repository: confirm it covers the code before E15-27 consumes it."
consumers = ["E15-27"]

[[source]]
id = "websight"
name = "HuggingFaceM4 WebSight synthetic pages"
kind = "dataset"
url = "https://huggingface.co/datasets/HuggingFaceM4/WebSight"
pinned = "b11f8172f89c992b56ac702319e02c428cca4a4e"
pin_source = "https://huggingface.co/api/datasets/HuggingFaceM4/WebSight/revision/b11f8172f89c992b56ac702319e02c428cca4a4e"
licence = "CC-BY-4.0"
use = "eval-only-cache"
attribution = "WebSight Dataset by HuggingFaceM4, licensed under CC BY 4.0."
review = "pending"
reviewer = ""
reviewed_on = ""
notes = "No owner ruling yet. CC BY 4.0 allows redistribution with attribution; the licence of any embedded photos must be confirmed before rendered fixtures are committed."
consumers = ["E15-27"]

[[source]]
id = "uicrit"
name = "Google Research UICrit critiques and ratings"
kind = "dataset"
url = "https://github.com/google-research-datasets/uicrit"
pinned = "adc92136cdaecf6a5c8bb85af08594dd9271eb00"
pin_source = "https://api.github.com/repos/google-research-datasets/uicrit/commits/adc92136cdaecf6a5c8bb85af08594dd9271eb00"
licence = "CC-BY-4.0"
use = "eval-only-cache"
attribution = "UICrit Dataset by Google Research (Duan et al., UIST 2024), licensed under CC BY 4.0."
review = "approved"
reviewer = "owner"
reviewed_on = "2026-10-19"
notes = "Owner ruling 2026-10-19: approved as eval-only-cache; only screen IDs and pair definitions are committed in uicrit_pairs.toml, while critiques and screenshots remain in the user cache. The repository README states CC BY 4.0; its screenshots come from RICO (entry rico-screenshots)."
consumers = ["E15-32"]

[[source]]
id = "rico-screenshots"
name = "RICO mobile UI screenshot corpus"
kind = "dataset"
url = "http://interactionmining.org/rico"
pinned = "gcs-1552078162173569"
pin_source = "https://storage.googleapis.com/crowdstf-rico-uiuc-4540/rico_dataset_v0.1/unique_uis.tar.gz"
licence = "LicenseRef-RICO-Terms"
use = "eval-only-cache"
attribution = "RICO: A Mobile App Dataset for Building Data-Driven Design Applications (Deka et al., UIST 2017)."
review = "approved"
reviewer = "owner"
reviewed_on = "2026-10-19"
notes = "Owner ruling 2026-10-19: approved as eval-only-cache only; fetched at run time into the user cache and never committed or redistributed. The pin is the Google Cloud Storage generation of rico_dataset_v0.1/unique_uis.tar.gz (header x-goog-generation, md5 bdcf99d4dc7f36a3396e57e306c75696, 6471262799 bytes), read 2026-10-19; RICO publishes no commit or version tag. RICO terms, verbatim from http://interactionmining.org/archive/rico/copyright.txt: \"The screenshots contained in the Rico dataset may contain copyrighted work. By downloading the Rico dataset (the \"Database\"), you (the \"Researcher\") hereby agree to the following terms and conditions: 1. The University of Illinois makes no representations or warranties regarding the Database, including but not limited to warranties of non-infringement or fitness for a particular purpose. 2. Researcher accepts full responsibility for his or her use of the Database and shall defend and indemnify the Rico team and the University of Illinois, including their employees, Trustees, officers and agents, against any and all claims arising from Researcher's use of the Database, including but not limited to Researcher's use of any copies of copyrighted images that he or she may create from the Database. 3. Researcher may provide research associates and colleagues with access to the Database provided that they first agree to be bound by these terms and conditions. 4. If Researcher is employed by a for-profit, commercial entity, Researcher's employer shall also be bound by these terms and conditions, and Researcher hereby represents that he or she is fully authorized to enter into this agreement on behalf of such employer.\""
consumers = ["E15-32"]

[[source]]
id = "uiclip"
name = "UIClip model weights (biglab/uiclip_jitteredwebsites-2-224-paraphrased)"
kind = "weights"
url = "https://huggingface.co/biglab/uiclip_jitteredwebsites-2-224-paraphrased"
pinned = "fec2aa208f96d831f430fd8f13cd9d5fa41bdef7"
pin_source = "https://huggingface.co/api/models/biglab/uiclip_jitteredwebsites-2-224-paraphrased/revision/fec2aa208f96d831f430fd8f13cd9d5fa41bdef7"
licence = "MIT"
use = "eval-only-cache"
attribution = "UIClip: A Data-driven Model for Assessing User Interface Design (Wu et al., UIST 2024), MIT License."
review = "approved"
reviewer = "owner"
reviewed_on = "2026-10-19"
notes = "Owner ruling 2026-10-19: approved as eval-only-cache only; weights are downloaded into the user cache when the optional aesthetic feature is run and never enter the Studio app binary (R4.13). Licence is the MIT tag of the Hugging Face model."
consumers = ["E15-31", "E15-37"]

[[source]]
id = "clip-vit-b32"
name = "OpenAI CLIP ViT-B/32 weights (openai/clip-vit-base-patch32)"
kind = "weights"
url = "https://huggingface.co/openai/clip-vit-base-patch32"
pinned = "3d74acf9a28c67741b2f4f2ea7635f0aaf6f0268"
pin_source = "https://huggingface.co/api/models/openai/clip-vit-base-patch32/revision/3d74acf9a28c67741b2f4f2ea7635f0aaf6f0268"
licence = "MIT"
use = "eval-only-cache"
attribution = "Copyright (c) 2021 OpenAI (MIT License)."
review = "approved"
reviewer = "owner"
reviewed_on = "2026-10-19"
notes = "Owner ruling 2026-10-19: approved as eval-only-cache only; ViT-B/32 weights are cached in the user cache for the optional aesthetic score and never bundled in the Studio binary (R4.13)."
consumers = ["E15-31", "E15-37"]

[[source]]
id = "shadcn-ui"
name = "shadcn/ui open component and token templates"
kind = "kit"
url = "https://github.com/shadcn-ui/ui"
pinned = "daaf156ee00ed91cc7e6bdd937c1b4973152d141"
pin_source = "https://api.github.com/repos/shadcn-ui/ui/commits/daaf156ee00ed91cc7e6bdd937c1b4973152d141"
licence = "MIT"
use = "eval-redistribute"
attribution = "Copyright (c) 2023 shadcn (MIT License)."
review = "approved"
reviewer = "owner"
reviewed_on = "2026-10-19"
notes = "Owner ruling 2026-10-19 (open kits for S2: shadcn/ui + IBM Carbon): S2 and the deterministic generator recreate shadcn/ui tokens and component structures as our own documents."
consumers = ["E15-24", "E15-28", "E15-29", "E15-30"]

[[source]]
id = "ibm-carbon"
name = "IBM Carbon Design System tokens and components"
kind = "kit"
url = "https://github.com/carbon-design-system/carbon"
pinned = "48192318ce862033bfdfde56aa0e28ba7555313a"
pin_source = "https://api.github.com/repos/carbon-design-system/carbon/commits/48192318ce862033bfdfde56aa0e28ba7555313a"
licence = "Apache-2.0"
use = "eval-redistribute"
attribution = "Carbon Design System by IBM and contributors. Licensed under the Apache License, Version 2.0."
review = "approved"
reviewer = "owner"
reviewed_on = "2026-10-19"
notes = "Owner ruling 2026-10-19 (open kits for S2: shadcn/ui + IBM Carbon): S2 and the deterministic generator recreate Carbon tokens and component structures as our own documents. The repository has no NOTICE file at this revision; keep any NOTICE text if a file is ever copied."
consumers = ["E15-24", "E15-28", "E15-29", "E15-30"]

[[source]]
id = "jakubkrehel-skills"
name = "jakubkrehel/skills UI craft rules"
kind = "skills"
url = "https://github.com/jakubkrehel/skills"
pinned = "4c66b1fa86e9ab70187b43d7da9b6fdb29f2ef63"
pin_source = "https://api.github.com/repos/jakubkrehel/skills/commits/4c66b1fa86e9ab70187b43d7da9b6fdb29f2ef63"
licence = "MIT"
use = "ship-in-skills"
licence_text_file = "docs/legal/licences/jakubkrehel-skills.txt"
attribution = "Portions of this folder are copied from or derived from jakubkrehel/skills (https://github.com/jakubkrehel/skills), commit 4c66b1fa86e9ab70187b43d7da9b6fdb29f2ef63, Copyright (c) 2026 Jakub Krehel, MIT License (see LICENSE)."
review = "approved"
reviewer = "owner"
reviewed_on = "2026-10-18"
notes = "Owner ruling 2026-10-18: adopt the MIT jakubkrehel/skills with attribution. Full MIT text sits in skills/vendor/jakubkrehel-skills/ and is embedded in THIRD_PARTY_LICENSES.md."
consumers = ["E15-46", "E15-47"]

[[source]]
id = "transitions-dev"
name = "transitions.dev skills (no licence published)"
kind = "skills"
url = "https://transitions.dev"
pinned = ""
licence = "NONE"
use = "excluded"
attribution = ""
review = "rejected"
reviewer = "owner"
reviewed_on = "2026-10-18"
notes = "Owner ruling 2026-10-18: skip anything without a licence. No licence file found upstream."
consumers = []

[[source]]
id = "ease-design"
name = "ease-design first-party knowledge"
kind = "knowledge"
url = "https://github.com/jangtrinh/ease-design"
pinned = "c8e85d1feb74928b781300eac8e1cfcef67d8ee3"
pin_source = "https://api.github.com/repos/jangtrinh/ease-design/commits/c8e85d1feb74928b781300eac8e1cfcef67d8ee3"
licence = "MIT"
use = "ship-in-skills"
licence_text_file = "docs/legal/licences/ease-design.txt"
attribution = "Copyright (c) 2026 ease-design contributors (MIT License)."
review = "approved"
reviewer = "owner"
reviewed_on = "2026-10-19"
notes = "Owner ruling (README-part2): the ease-design knowledge set ships in the skills pack; the repository is the owner's own."
consumers = ["E15-18", "E15-21", "E15-31", "E15-47"]

[[source]]
id = "font-inter"
name = "Inter typeface"
kind = "font"
url = "https://github.com/rsms/inter"
pinned = "66647c0bbbe41a850d79d9c76fb13add3378940f"
pin_source = "https://api.github.com/repos/rsms/inter/commits/66647c0bbbe41a850d79d9c76fb13add3378940f"
licence = "OFL-1.1"
use = "ship-in-app"
licence_text_file = "crates/designos-fonts/assets/inter/OFL.txt"
attribution = "Copyright 2020 The Inter Project Authors (SIL Open Font License 1.1)."
review = "pending"
reviewer = ""
reviewed_on = ""
notes = "Already bundled (crates/designos-fonts/assets/README.md: version 4.001, Version 4.001;git-66647c0bb, taken from the Google Fonts repository). The pin is the upstream commit named in that version string. No owner ruling yet: confirm the bundled font."
consumers = ["E15-18"]

[[source]]
id = "figma-client-files"
name = "Proprietary client and user-home design files"
kind = "dataset"
url = "https://figma.com"
pinned = ""
licence = "NONE"
use = "excluded"
attribution = ""
review = "rejected"
reviewer = "owner"
reviewed_on = "2026-10-18"
notes = "Owner ruling 2026-10-18: the eval never uses client files; only open kits and self-generated large files. The [ban] patterns below enforce it."
consumers = []

[ban]
patterns = ['(?i)\bvsf\b', '(?i)\bpcp\b', '(?i)untitled ui', '^/Users/', '^/home/', '^[A-Za-z]:\\Users\\']
```
