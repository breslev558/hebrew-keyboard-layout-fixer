AUDIT TECHNIQUE — HEBREW KEYBOARD LAYOUT FIXER
Date : 9 novembre 2026
Dépôt : https://github.com/breslev558/hebrew-keyboard-layout-fixer
Révision examinée : 870c97ae394d923f61ec15035d716abdfd035bbe
Version publiée examinée : v1.5.3

1. AVIS GÉNÉRAL ET PORTÉE

Le projet possède de bonnes bases : séparation entre logique pure et Win32, dépendances verrouillées, tests des scénarios d'erreur, limites de taille du presse-papiers, gestion des raccourcis occupés, installation par utilisateur, attribution du projet d'origine et archive accompagnée d'empreintes.

Il reste plusieurs défauts fonctionnels et des limites de protection à traiter avant de présenter le logiciel comme entièrement fiable pour tous les contenus et toutes les applications. Les priorités sont la conservation du presse-papiers, la stabilité de la cible et les tests réels d'intégration Windows.

Cet audit est une revue approfondie des sources et de la distribution, complétée par des vérifications ciblées. Ce n'est pas une certification de sécurité ou une validation exhaustive du comportement Windows. Aucun logiciel n'est démontré « idéal » par un audit ponctuel.

Le dépôt contient 30 fichiers Rust, environ 9 509 lignes non vides, et 128 attributs #[test], dont un associé à un test explicitement ignoré. Ce comptage décrit les sources ; il ne représente pas 127 tests exécutés avec succès.

Vérifications réellement réalisées :
- Lecture des chemins de conversion, de la provenance du presse-papiers, de l'intégration Win32, de l'installation, des paramètres, profils et politiques.
- Consultation de l'arborescence, des deux commits disponibles, de la release, des exécutions GitHub Actions et des rulesets publics.
- Téléchargement et inspection de l'archive publiée, sans exécuter le programme.
- Vérification SHA-256 de l'archive et de l'exécutable, métadonnées de version, signature Authenticode et comparaison des scripts distribués avec les sources.
- Analyse syntaxique PowerShell des deux installateurs : aucune erreur détectée.
- Exécution du bloc exact de remplacement/retour arrière de l'installateur dans un dossier isolé, avec échec simulé du registre ; aucun vrai registre modifié.
- Vérifications structurelles des différences de profils ; modélisation fidèle du cas de saturation des règles administrateur.
- Requête OSV sur les 33 dépendances de registre du Cargo.lock : aucune vulnérabilité connue renvoyée pour ces versions au moment de la requête.
- Recherche de signatures courantes de secrets dans les deux états de commits : aucun résultat pour clés privées, jetons GitHub, identifiants AWS AKIA et certaines formes de clés sk-. Ce contrôle limité n'exclut pas tout secret.

Non exécutés : cargo test, cargo clippy, cargo fmt, compilation Windows, reconstruction indépendante de la release, essais interactifs Word/Excel/navigateurs, installation complète et désinstallation complète. Rust, Cargo, Python et Bash ne sont pas disponibles dans l'environnement de cet audit. Le logiciel n'a pas été lancé afin de ne pas enregistrer ses raccourcis globaux ou perturber le presse-papiers de la session utilisée.

Niveaux : P1 = conservation des données à traiter en premier ; P2 = défaut fonctionnel ou fiabilité à corriger ; P3 = amélioration de publication/usage. « Confirmé par lecture » signifie que la branche fautive est identifiée dans le code, sans prétendre avoir reproduit son effet dans une application Windows.

2. CONSTATS PRIORITAIRES

A01 — P1 — La sauvegarde du presse-papiers n'est pas complète
Statut : comportement confirmé par lecture ; conséquences Office à valider.
Source : crates/klf-win/src/platform/clipboard.rs, lignes 25–28, 90–99, 198–200 et 225–238.
Plusieurs formats OLE, dont Embed Source, Embedded Object et Object Descriptor, sont ignorés volontairement. Des formats enregistrés illisibles sont également abandonnés. Si d'autres formats peuvent être sauvegardés, le presse-papiers est ensuite vidé et seuls les éléments conservés sont restaurés.
Scénario : copier un objet incorporé qui propose une représentation texte/image et une représentation OLE, puis corriger un autre texte avec F10. La copie peut revenir comme texte ou image tout en perdant la possibilité de recoller l'objet éditable. La perte de format découle du chemin de code ; l'application et le format exacts doivent être couverts par un essai Windows.
Correction : définir explicitement les garanties par format. Si un format utile ne peut pas être préservé, refuser l'opération avant EmptyClipboard. Étudier une stratégie OLE/IDataObject adaptée au lieu de recopier aveuglément des handles. Ne pas supprimer les exclusions OLE sans vérifier les contrats de propriété. Ajouter des essais de collage spécial Office, objets incorporés, fichiers, images et données à rendu différé. Préciser les limites dans le README.

A02 — P1 — Une nouvelle copie de la même fenêtre peut être écrasée
Statut : limite explicitement acceptée par les sources et les tests existants.
Source : crates/klf-core/src/flow.rs, lignes 127–138, 1760–1771 et 1812–1828.
Quand le numéro de séquence change, is_app_copy accepte encore le contenu si le propriétaire et le texte sont identiques. Sans texte, il suffit que le propriétaire et la liste des formats soient identiques. Deux images différentes copiées depuis la même fenêtre peuvent donc être indistinguables pour cette règle. La restauration peut écraser une nouvelle copie avec l'ancien contenu sauvegardé.
Le test audit133_original_r3_strict_property_remains_unfulfilled est ignoré ; un autre test affirme expressément que deux copies d'images de même propriétaire et mêmes formats restent acceptées.
Correction : rendre le compromis configurable ou choisir un mode strict qui annule en cas de séquence ambiguë. Comparer le contenu sauvegardable des formats peut réduire l'ambiguïté sur les images, mais ne distingue pas deux copies strictement identiques. Définir les limites restantes et éviter toute promesse absolue de conservation des nouvelles copies.

A03 — P2 — La cible et le curseur sont mémorisés trop tard pour certains mouvements
Statut : risque déduit des sources ; reproduction Windows nécessaire.
Sources : crates/klf-win/src/app.rs, vers 810–818 ; crates/klf-core/src/flow.rs, 409–467 et 546 ; crates/klf-win/src/platform/input.rs, 117–123.
La fenêtre cible est choisie sur le thread de travail, après la réception du raccourci. Le point de référence du curseur est mémorisé après la dernière copie et son attente de stabilisation. La détection user_moving ne suit que les boutons de souris, Tab et Entrée, et ne couvre pas les flèches ni la frappe ordinaire.
Un changement de fenêtre avant le démarrage du worker peut faire choisir une autre cible. Un déplacement au clavier pendant la copie peut devenir le nouveau point de référence et passer les vérifications suivantes. Le contrôle final du focus apporte une protection utile, mais ne démontre pas la stabilité de toute la sélection depuis le raccourci.
Correction : transmettre la cible initiale depuis le thread UI, mémoriser les informations pertinentes avant chaque copie, puis vérifier les mouvements après les attentes. Distinguer les mouvements voulus, comme Shift+Home, des mouvements externes. Ajouter des tests déterministes de changement de cible et des essais avec flèches/frappe pendant la phase Ctrl+C.

A04 — P2 — Le retour arrière de la première installation annonce trop
Statut : reproduit en exécutant le bloc original dans un dossier de test.
Source : install/Installer.ps1, lignes 70–84 et 92–93.
Lors d'une première installation, le nouveau fichier est déplacé vers son nom définitif, puis la clé Run est créée. Si cette étape de registre échoue, le catch ne supprime pas le nouvel exécutable : il ne restaure .old que si l'exécutable définitif n'existe pas. Le message affirme pourtant que l'état précédent a été remis (exeStillPresent=true après l'échec injecté, message « The previous state was put back »).
En outre, .old est supprimé avant Start-Process : un échec de lancement n'est pas couvert par le mécanisme de retour arrière.
Correction : suivre les opérations réellement effectuées, annuler celles-ci en sens inverse et adapter le message au résultat. Conserver l'ancien fichier jusqu'à la validation du lancement. Tester les échecs de copie, registre, échange de fichiers, raccourci et lancement.

A05 — P2 — La saturation des règles utilisateur peut écarter une règle machine
Statut : confirmé par lecture et par modèle fidèle de l'insertion bornée.
Sources : crates/klf-win/src/platform/school.rs, lignes 82–85 ; crates/klf-core/src/school.rs, lignes 136–161 ; MAX_RULES=200.
Les valeurs HKCU sont lues avant HKLM. Après 200 règles distinctes venant de HKCU, une nouvelle règle HKLM ne peut plus être ajoutée. Une règle machine concernant un autre programme est silencieusement perdue, ce qui contredit la priorité machine annoncée.
Correction : analyser les sources séparément, réserver la capacité aux règles machine puis compléter avec les règles utilisateur, et signaler toute troncature. Ajouter le cas « 200 HKCU + nouvelle règle HKLM » aux tests du cœur.

A06 — P2 — L'import ignore certaines différences réelles de profil
Statut : confirmé par lecture et contrôle structurel.
Sources : crates/klf-core/src/profile.rs, lignes 42–74 ; crates/klf-win/src/settings_ui.rs, lignes 854–858.
ReplaceCaps est exporté/importé et utilisé pour convertir le texte, mais differences ne le compare pas. Si ce seul réglage change, l'interface annonce qu'il n'y a rien à importer. Si un autre réglage change aussi, ReplaceCaps est modifié sans apparaître dans la confirmation. RunAtStartup est également sérialisé sans figurer dans les différences ; le démarrage réel dépend du registre. FormatVersion est exporté mais n'est pas validé à l'import.
Correction : définir les champs transportés par un profil, comparer tous les champs fonctionnels et refuser les versions de format non prises en charge. Tester un import pour chaque champ modifié isolément, notamment ReplaceCaps.

A07 — P2 — Enregistrer les paramètres peut retirer Win du raccourci F9
Statut : confirmé par lecture ; essai interactif à ajouter.
Source : crates/klf-win/src/settings_ui.rs, lignes 360–369 et 638–671.
Le contrôle natif de raccourci ne restitue pas un modificateur Win. Le code compense pour le raccourci principal inchangé, mais pas pour celui des majuscules. Un raccourci secondaire Win+F9 peut devenir F9 lorsque l'on enregistre un autre paramètre.
Correction : appliquer la même conservation aux deux raccourcis ou proposer une saisie qui représente Win explicitement. Tester « ouvrir puis enregistrer sans changer les raccourcis » pour les deux actions et chaque modificateur.

A08 — P2 — Après les derniers échecs de restauration, la sauvegarde est perdue
Statut : confirmé par lecture ; scénario de contention prolongée à tester.
Sources : crates/klf-core/src/flow.rs, lignes 377–399 ; crates/klf-win/src/host.rs, clipboard_restore ; app.rs, fin du worker.
La sauvegarde reste attachée au WinHost local et est détruite à la fin du worker. Le message d'erreur n'offre pas de récupération du contenu initial si le presse-papiers redevient disponible ensuite.
Correction : conserver temporairement en mémoire une sauvegarde récupérable, avec action explicite de nouvelle tentative et vérification de provenance. Ne pas la restaurer automatiquement par-dessus une copie ultérieure. Durée de rétention courte, jamais sur disque sans décision et consentement.

3. QUALITÉ DU DÉPÔT ET DE LA DISTRIBUTION

A09 — P2 — Les tests ne sont pas vérifiés automatiquement sur GitHub
Aucun workflow .github/workflows ; zéro exécution. TESTS.md ligne 3 mentionne tests-ps/test-installeur-p1-p3.ps1, absent du dépôt. Ligne 13 : Désinstaller.cmd alors que le fichier publié est Uninstall.cmd.
Ajouter une CI sur push et pull_request : tests du cœur, compilation Windows, fmt/clippy après état de référence, analyse de dépendances, tests PowerShell isolés. Commandes : cargo test --locked -p klf-core ; cargo fmt --all -- --check ; cargo clippy --locked --workspace --all-targets -- -D warnings ; cargo build --locked --release --target x86_64-pc-windows-gnu -p KeyboardLayoutFixer. Documenter MinGW ; éviter la dépendance à des outils locaux non documentés.

A10 — P3 — La release est traçable, mais pas reconstruite indépendamment
Archive KeyboardLayoutFixer.zip 339 462 octets, SHA-256 32c84d58246c44bacfd821323b911cc64170dd71d96c2bb9d09d97aadd6977b8 ; exe 626 176 octets, SHA-256 bcee0168ff750d11f6d956e690890161b9653f8c16fbcbd2808357092e6f7cd7 ; FileVersion 1.5.3.0 ; ProductVersion 1.5.3 ; BUILD-INFO correct ; scripts identiques aux sources ; Authenticode NotSigned.
Amélioration : publier depuis une CI contrôlée, conserver les logs, décrire les outils, attestation de provenance (GitHub artifact attestations). Présenter la vérification d'origine avant les instructions de contournement de SmartScreen.

A11 — P3 — La localisation française/anglaise/hébraïque est incomplète
Boîtes de sélection de fichiers, erreurs de profils, liste des différences et une grande partie du diagnostic restent en français quel que soit UiLanguage (settings_ui.rs:783, profile.rs:21 et :42, diag.rs:233). Scripts d'installation en anglais. README plus détaillé en anglais.
Centraliser dans i18n, traduire refus et filtres, vérifier RTL, focus clavier, textes longs.

A12 — P3 — Les garanties utilisateur devraient être formulées plus précisément
Le README annonce la protection des champs de mot de passe : seule ES_PASSWORD sur Edit/RichEdit est détectée explicitement ; le reste repose sur le refus de Ctrl+C par l'application. Le contrôle des fenêtres élevées est neutralisé quand le programme tourne lui-même élevé. Consoles classiques activables en option.
Documenter cas pris en charge/refusés/limites ; ne pas présenter la protection comme absolue ; expliquer que F10 corrige la disposition physique sans détecter l'intention linguistique, et que la ponctuation seule n'est pas toujours réversible. Préciser les métadonnées du diagnostic (noms d'exécutables, classes de fenêtres, tailles de texte, paramètres).

A13 — P3 — Faciliter la maintenance et la désinstallation
L'installateur copie seulement l'exe ; les scripts de désinstallation restent dans le dossier extrait. Ajouter une entrée de désinstallation Windows et un script installé à un emplacement défini. Guide de compilation (Rustup, MinGW/binutils, Bash, Python), guide de contribution, canal privé de signalement de vulnérabilités (SECURITY.md), changelog, modèles de bug sans texte confidentiel. Séparer les tests de flow.rs. Nettoyer le .gitignore .NET. Remplacer le parseur SPDX maison par cargo-deny ; vérifier les notices de licences des dépendances.

4. POINTS RÉUSSIS À CONSERVER
Architecture klf-core / klf-win ; contrats Host/Capture/ClipState ; attente limitée des copies, sauvegarde bornée, allocation avant publication, contrôle de propriété ; protection éditeurs avec terminal, exclusion apps distantes/VM/gestionnaires de mots de passe ; console désactivée par défaut ; paramètres via fichier temporaire ; F9 Unicode ; accessibilité/contraste/DPI/RTL ; installation par utilisateur ; MIT + attribution ; Cargo.lock, Rust fixé, BUILD-INFO et hash concordants.

5. PLAN DE CORRECTION CONSEILLÉ
Lot 1 : A01/A02, tests de régression A03, récupération en mémoire A08 ; valider avec Word/Excel/Chrome/Edge et presse-papiers réels (OLE, copies concurrentes).
Lot 2 : A04 à A07 avec tests précis (échec registre après échange, 200 règles HKCU, profil ne changeant que ReplaceCaps, Win+F9 inchangé).
Lot 3 : CI, références de tests à jour, tests du cœur + essais Windows, fiche TESTS.md remplie, publier depuis le commit testé.
Lot 4 : localisation, documentation des limites, guide de build, désinstallation, changelog, provenance, signature selon les moyens.
Aucune vulnérabilité critique d'exécution de code établie. Les deux P1 concernent la conservation des données.
