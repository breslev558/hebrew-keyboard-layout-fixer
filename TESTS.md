# Essais Windows à faire à chaque version

*Manual test checklist for each release (in French).*

Les tests automatiques (`cargo test -p klf-core`, `tests-ps/*.ps1`, lancés aussi par la CI GitHub à chaque envoi) simulent
Windows. Cette liste se fait à la main sur un PC d'essai, avec la version
indiquée. Cocher, noter la date et ce qui ne va pas. Rien de ce qui est tapé n'est enregistré par
le programme : pour signaler un problème, utiliser l'onglet **Diagnostic** puis **Copier les
informations**.

Version : ______  PC : ______  Date : ______  Windows : ______  Échelle : ______

## Installation
- [ ] install/Installer.cmd par-dessus l'ancienne version : fermée, remplacée, relancée, réglages gardés.
- [ ] Uninstall.cmd (ou Paramètres > Applications installées) : tout est retiré, ou ce qui reste est dit.

## F10 (langue) et F9 (majuscules)
- [ ] Word : sélection « akuo » → « שלום » ; sans sélection : jusqu'au début de la ligne.
- [ ] Excel en édition de cellule (F2) : sélection corrigée ; cellule entière copiée = refus.
- [ ] Navigateur (Chrome, Edge) avec deux champs : correction dans le bon champ ; changer de champ pendant la correction = arrêt.
- [ ] Champ mot de passe d'une fenêtre Windows : jamais corrigé.
- [ ] Presse-papiers image (capture d'écran) puis F10 : l'image est remise après.
- [ ] Presse-papiers fichiers (copie dans l'explorateur) puis F10 : les fichiers sont remis.
- [ ] F9 : « hELLO » → « Hello » ; Verr. Maj éteint après seulement s'il était allumé.
- [ ] F9 avec Verr. Maj pressé pendant la correction : Verr. Maj n'est pas rallumé.
- [ ] Defender actif : aucune alerte au démarrage ni à F9 (crochet clavier limité à Verr. Maj).

## Claviers (1.4.0)
- [ ] Hébreu + anglais (États-Unis) : comme avant.
- [ ] Hébreu + français (AZERTY) : « qkuo » → « שלום », « נםמחםור » → « bonjour » ; chiffres et é è ç à inchangés.
- [ ] Anglais (Royaume-Uni) ou États-Unis international actif : F10 marche (table anglais États-Unis), le diagnostic le signale.
- [ ] Variante hébraïque (Hebrew Standard SI-1452) active : F10 marche si ses lettres sont aux mêmes places.
- [ ] Clavier russe ou arabe actif : F10 refuse (« Clavier actif non pris en charge »), rien n'est changé.
- [ ] « Changer la langue après » : bascule vers le bon clavier (hébreu, ou le clavier latin utilisé).

## Fenêtre de réglages (1.4.0)
- [ ] 100 %, 150 %, 200 % : rien n'est coupé ; à 200 % sur un écran de 768 px, la fenêtre tient (zones de texte réduites).
- [ ] Contraste élevé (Alt+Maj gauche+Impr. écran) : tout reste lisible, ligne d'état et bulle comprises.
- [ ] Tout au clavier : Tab, Maj+Tab, Ctrl+Tab (onglet suivant), Ctrl+Maj+Tab, Entrée = Enregistrer, Échap = Fermer.
- [ ] Hébreu : fenêtre de droite à gauche, onglets à droite.
- [ ] Atelier : texte collé → résultat en direct ; les 4 choix ; « Copier le résultat » ; rien n'est gardé après fermeture.
- [ ] Atelier : sélectionner une partie du texte puis F10, puis F9 : seule la sélection change, le reste non.
- [ ] Onglet Avancé : mode prudent, Verr. Maj, console, limite, délai, programmes exclus enregistrés et relus.
- [ ] Onglet Diagnostic : la dernière tentative est expliquée ; « Copier les informations » ne contient ni texte tapé, ni titre de document, ni nom de compte.

## Dernier mot (1.4.1-essai, case « Sans sélection : seulement le dernier mot »)
- [ ] Bloc-notes : « bonjour akuo » + F10 sans sélection → seul « akuo » devient « שלום ».
- [ ] Word (paragraphe de gauche à droite) : idem ; mot avec apostrophe (« akuo' » devenue ’) → « שלום, » ; presse-papiers remis.
- [ ] Word paragraphe hébreu (de droite à gauche) : refus « Sélectionnez le texte », rien n'est collé, presse-papiers remis.
- [ ] Chrome/Edge (champ, zone de texte, éditeur riche) : dernier mot corrigé, ou refus sans dégât.
- [ ] Excel : la case n'a pas d'effet (ligne entière comme avant, ou refus).
- [ ] Maj, Ctrl, Alt ou Win encore enfoncée : refus.

## Menu de l'icône (1.4.0)
- [ ] Dans Word, clic droit sur l'icône : « Désactiver dans winword.exe » → F10 ne fait plus rien dans Word, marche ailleurs ; « Réactiver ».
- [ ] « Seulement avec une sélection dans winword.exe » : sans sélection, F10 demande de sélectionner.
- [ ] Application du Store (Calculatrice, Bloc-notes Windows 11) : la règle vise l'application, pas « applicationframehost.exe ».
- [ ] « Pause pendant 10 minutes » : F10 et F9 libres pour les autres programmes ; reprise seule après 10 minutes ; si F10 est pris entre-temps, une bulle le dit.
- [ ] Un champ mot de passe reste protégé même avec une règle.

## Indicateur (1.5.0, case dans l'onglet Simple)
- [ ] Case cochée : Alt+Maj dans le Bloc-notes → « HE » / « EN » près du curseur, sans prendre la main.
- [ ] Changer de fenêtre (langue différente) → l'étiquette de la nouvelle langue.
- [ ] Verr. Maj → « VERR. MAJ », puis « verr. maj éteint ».
- [ ] Vidéo ou présentation en plein écran : rien.
- [ ] Partage d'écran Teams / capture (Win+Maj+S) : l'étiquette n'apparaît pas dans l'image.
- [ ] Application du Store (Calculatrice) : la bonne langue, ou rien.
- [ ] Case décochée : plus rien.

## Réglages imposés par l'administrateur (1.5.0)
- [ ] des valeurs dans HKLM\SOFTWARE\Policies\NOAM\KeyboardLayoutFixer (en administrateur) sur le PC d'essai : en moins d'une minute, l'indicateur se coupe, les cases imposées sont grisées, « gérés par l'administrateur » dans la ligne d'état, la règle « jeu.exe » cochée et grisée dans le menu.
- [ ] Un utilisateur standard (compte non administrateur) ne peut pas changer ces réglages.
- [ ] Valeurs retirées du registre : chacun retrouve ses réglages.
- [ ] Valeur fausse (ex. PasteDelayMs = 5) : ignorée, signalée dans l'onglet Diagnostic.
- [ ] Exporter, modifier un réglage, Importer : différences montrées, réglages remis ; « Revenir à avant ».

## Performances (onglet Diagnostic, durées par étape)
- [ ] Noter la durée totale d'une correction courte dans Word : ______ ms (dont attente avant remise : réglage « délai »).
- [ ] Noter une correction dans Chrome : ______ ms. Comparer d'une version à l'autre.

## Audit du 09/11/2026 (1.5.4)
- [ ] A01 : copier un objet Word/Excel incorporé, faire F10 sur un autre texte, puis Collage spécial : noter ce qui revient (limite connue : formats OLE non gardés).
- [ ] A02 : copier une image, F10, recopier une AUTRE image dans la même fenêtre pendant la seconde de remise : noter si la nouvelle est gardée (limite connue).
- [ ] A03 : appuyer sur F10 puis tenir une flèche pendant la copie : la correction s'arrête (« fenêtre changée » ou rien), le texte n'est pas collé ailleurs.
- [ ] A07 : régler Win+F9 (fichier settings.json), ouvrir les réglages, Enregistrer sans toucher aux touches : Win+F9 reste.
- [ ] A08 : bloquer le presse-papiers (programme qui le garde ouvert) pendant F10 : message « presse-papiers non remis », puis menu de l'icône « Remettre le presse-papiers précédent » (moins de 10 minutes) : l'ancienne copie revient ; après une nouvelle copie, rien n'est remis.
- [ ] A04/A13 (version publique) : installer, vérifier « Paramètres > Applications > Applications installées > Keyboard Layout Fixer », désinstaller depuis là ; les autres programmes de « Démarrage » sont toujours là.

## 1.6.0
- [ ] Version publique : télécharger KeyboardLayoutFixer-Setup.exe, double-clic, « Exécuter quand même » : installé, icône près de l'horloge, fenêtre de bienvenue ; « Essayer maintenant » ouvre la zone d'essai ; « Ne plus afficher » : plus jamais au redémarrage.
- [ ] Bulle « akuo → שלום » après F10 (hébreu et latin dans le bon ordre, interface en hébreu aussi) ; menu de l'icône « Montrer le texte corrigé » décoché : bulle « Corrigé » seulement.
- [ ] Champ mot de passe (fenêtre Windows et page web) : jamais de bulle avec du texte.
- [ ] Installeur lancé pendant que le programme tourne : fermé, remplacé, relancé ; Paramètres › Applications installées › Désinstaller : tout retiré, réglages gardés.
- [ ] PC géré par un administrateur (installation dans Program Files ou clé de politique) : pas de fenêtre de bienvenue, pas de mise à jour.
