import { sharedRepositoryContractTests } from "@webglobe/core/src/storage/repositoryContractTests.js";
import { IndexedDbPlacesRepository } from "../src/IndexedDbPlacesRepository.js";

let dbCounter = 0;

sharedRepositoryContractTests(() => new IndexedDbPlacesRepository(`webglobe-test-${dbCounter++}`));
