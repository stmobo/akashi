# Akashi

[![docs](https://docs.rs/akashi/badge.svg)](https://docs.rs/akashi/0.1.0/akashi/)
[![tests](https://github.com/stmobo/akashi/workflows/tests/badge.svg)](https://github.com/stmobo/akashi)

A framework for building collectible card games and gacha games.

## Work-in-progress

Akashi is very much a work in progress framework right now. There are
plenty of rough edges and hard-to-use parts here, and there's plenty of
distance to cover before Akashi can be considered ready for real use.

## Overview

Akashi aims to give developers an easy framework to build games based
around collectible cards and/or gacha mechanics. It also aims to add
ready-to-go implementations of, and building blocks for, common
mechanics to make it even easier to get started.

It draws some inspiration from traditional game engines, but with tweaks
in order to better fit aspects associated with collection games.

## Architecture

Akashi uses an Entity-Component-System architecture (though at the moment
only Entities and Components are really implemented).

Players and cards, within the Akashi framework, are **entities**: they
aren't much more than a unique ID. Functionality is added by attaching
various **components** to entities.
For example, inventories can be represented as components that are
attached to players, while card images and text can be represented as
components attached to cards.
