import { sharedRepositoryContractTests } from "@webglobe/core";
import { IndexedDbPlacesRepository } from "../src/IndexedDbPlacesRepository.js";

let dbCounter = 0;

sharedRepositoryContractTests(() => new IndexedDbPlacesRepository(`webglobe-test-${dbCounter++}`));
